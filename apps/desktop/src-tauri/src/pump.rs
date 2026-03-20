use std::sync::Arc;
use std::time::Instant;

use mapping_core::engine::{EngineOutput, RawTapEvent};
use mapping_core::types::{Action, Modifier, MouseButton, ScrollDirection, VibrationPattern};
use tauri::{Emitter as _, Manager as _};
use tokio::sync::broadcast;

use tokio::sync::watch;

use crate::{
    events::{
        ActionFiredPayload, ContextRuleMatchedPayload, LayerChangedPayload, TapEventPayload,
        ACTION_FIRED, CONTEXT_RULE_MATCHED, DEBUG_EVENT, LAYER_CHANGED, TAP_EVENT,
    },
    state::AppState,
};

// ── Event pump task ───────────────────────────────────────────────────────────

/// Long-running task that drives the combo engine from BLE tap events.
///
/// Uses a dynamic sleep deadline (via [`ComboEngine::next_deadline`]) rather
/// than a fixed polling interval so that buffered taps are flushed as soon as
/// their detection window expires — eliminating up to 50 ms of extra latency
/// that the previous fixed-tick approach introduced.
///
/// Does not hold an `Enigo` handle directly — keyboard simulation is dispatched
/// via [`simulate_key`] which calls `spawn_blocking` so that `Enigo` (which may
/// be `!Send`) lives only within the blocking thread.
pub async fn run_event_pump(
    app: tauri::AppHandle,
    state: Arc<AppState>,
    mut event_rx: broadcast::Receiver<RawTapEvent>,
) {
    loop {
        // Recompute the earliest pending deadline each iteration so that newly
        // buffered events are picked up without an extra round-trip.
        let next_deadline = state.engine.lock().await.next_deadline();

        let timeout_fut = async {
            match next_deadline {
                Some(d) => tokio::time::sleep_until(d.into()).await,
                // Nothing pending — park until an event arrives.
                None => std::future::pending::<()>().await,
            }
        };

        tokio::select! {
            result = event_rx.recv() => {
                match result {
                    Ok(raw_event) => {
                        let _ = app.emit(TAP_EVENT, TapEventPayload::from(&raw_event));
                        let outputs = {
                            let mut engine = state.engine.lock().await;
                            engine.push_event(raw_event, Instant::now())
                        };
                        if !outputs.is_empty() {
                            maybe_haptic_on_tap(&state).await;
                        }
                        process_outputs(&app, &state, outputs).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("event pump lagged, dropped {n} tap events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = timeout_fut => {
                let outputs = {
                    let mut engine = state.engine.lock().await;
                    engine.check_timeout(Instant::now())
                };
                if !outputs.is_empty() {
                    maybe_haptic_on_tap(&state).await;
                    process_outputs(&app, &state, outputs).await;
                }
            }
        }
    }
}

// ── BLE status listener task ──────────────────────────────────────────────────

/// Relays BLE connect/disconnect notifications to the Svelte frontend.
pub async fn run_ble_status_listener(
    app: tauri::AppHandle,
    mut status_rx: broadcast::Receiver<tap_ble::BleStatusEvent>,
) {
    // Keep a reference to AppState so we can read device count for tray updates.
    let state = app.try_state::<std::sync::Arc<crate::state::AppState>>();

    while let Ok(event) = status_rx.recv().await {
        let (event_name, payload) = ble_status_to_event(&event);
        let _ = app.emit(event_name, payload);

        if let Some(ref state_arc) = state {
            // Send OS notification if the relevant preference is enabled.
            {
                let prefs = state_arc.preferences.lock().await;
                let (enabled, title, body) = match &event {
                    tap_ble::BleStatusEvent::Connected { device_id, name, .. } => {
                        let role = capitalize_role(&device_id.to_string());
                        let label = device_label(name.as_deref(), &role);
                        (
                            prefs.notify_device_connected,
                            "Device Connected",
                            format!("{label} is ready"),
                        )
                    }
                    tap_ble::BleStatusEvent::Disconnected { device_id, name, .. } => {
                        let role = capitalize_role(&device_id.to_string());
                        let label = device_label(name.as_deref(), &role);
                        (
                            prefs.notify_device_disconnected,
                            "Device Disconnected",
                            format!("{label} disconnected"),
                        )
                    }
                };
                if enabled {
                    crate::send_notification(&app, title, &body);
                }
            }
            update_tray_from_state(&app, state_arc).await;
        }
    }
}

fn ble_status_to_event(
    event: &tap_ble::BleStatusEvent,
) -> (&'static str, crate::events::DeviceStatusPayload) {
    match event {
        tap_ble::BleStatusEvent::Connected { device_id, address, .. } => (
            crate::events::DEVICE_CONNECTED,
            crate::events::DeviceStatusPayload {
                role: device_id.to_string(),
                address: address.to_string(),
            },
        ),
        tap_ble::BleStatusEvent::Disconnected { device_id, address, .. } => (
            crate::events::DEVICE_DISCONNECTED,
            crate::events::DeviceStatusPayload {
                role: device_id.to_string(),
                address: address.to_string(),
            },
        ),
    }
}

// ── Context monitor task ──────────────────────────────────────────────────────

/// Long-running task that watches for focused-window changes and activates
/// the first matching context rule.
///
/// `_cancel_tx` keeps the cancel channel alive for the lifetime of this task;
/// when the task is dropped (app shutdown) the sender is dropped too, which
/// signals all background OS threads to exit.
pub async fn run_context_monitor(
    app: tauri::AppHandle,
    state: Arc<AppState>,
    cancel_rx: watch::Receiver<bool>,
    _cancel_tx: watch::Sender<bool>,
) {
    let mut monitor_rx = crate::focus_monitor::start_monitor(cancel_rx);

    loop {
        // Wait until the focused window changes.
        if monitor_rx.changed().await.is_err() {
            // Sender dropped — monitor shut down (e.g. no display available).
            break;
        }

        let window = match monitor_rx.borrow().clone() {
            Some(w) => w,
            None => continue,
        };

        // Step 1: snapshot the active layer id — hold engine lock only briefly.
        let active_layer_id = {
            let ids = state.engine.lock().await.layer_ids();
            ids.into_iter().next().unwrap_or_default()
        };

        // Step 2: evaluate context rules — hold context_rules lock only briefly.
        let matched = {
            let rules = state.context_rules.lock().await;
            rules.evaluate(&window, &active_layer_id).cloned()
        };

        let Some(rule) = matched else { continue };

        // Step 3: resolve the profile from the registry.
        let profile = {
            let reg = state.layer_registry.lock().await;
            reg.get(&rule.layer_id).cloned()
        };

        let Some(profile) = profile else {
            log::warn!(
                "context_monitor: rule '{}' references unknown layer_id '{}'",
                rule.name,
                rule.layer_id
            );
            continue;
        };

        let layer_id = rule.layer_id.clone();
        let rule_name = rule.name.clone();

        // Guard: skip if this profile is already active.
        //
        // The Wayland monitor fires on every toplevel `Done` event, which includes
        // window title changes of the currently-focused window (e.g. browser tab
        // updates). Without this check, a matching context rule would re-apply the
        // same profile and re-fire haptics on every such event.
        {
            let prefs = state.preferences.lock().await;
            if prefs.last_active_profile_id.as_deref() == Some(layer_id.as_str()) {
                continue;
            }
        }

        // Step 4: activate the profile.
        state.engine.lock().await.set_profile(profile);

        {
            let mut prefs = state.preferences.lock().await;
            prefs.profile_active = true;
            prefs.last_active_profile_id = Some(layer_id.clone());
            if let Err(e) = prefs.save(&state.preferences_path) {
                log::warn!("context_monitor: failed to save preferences: {e}");
            }
        }

        emit_layer_changed(&app, &state).await;
        maybe_notify_profile_switch(&app, &state).await;
        maybe_haptic_on_profile_switch(&state).await;
        let _ = app.emit(
            CONTEXT_RULE_MATCHED,
            ContextRuleMatchedPayload {
                rule_name,
                layer_id,
            },
        );

        log::info!(
            "context_monitor: activated '{}' via rule '{}'",
            rule.layer_id,
            rule.name
        );
    }
}

// ── Notification helpers ──────────────────────────────────────────────────────

/// Capitalise the first character of a device role string ("left" → "Left").
fn capitalize_role(role: &str) -> String {
    let mut chars = role.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Format the device label as "Name (Role)" when a name is available,
/// or just "Role" when it is not.
fn device_label(name: Option<&str>, role: &str) -> String {
    match name {
        Some(n) if !n.is_empty() => format!("{n} ({role})"),
        _ => role.to_string(),
    }
}

/// Send a layer-switch OS notification if `notify_layer_switch` is enabled.
pub(crate) async fn maybe_notify_layer_switch(app: &tauri::AppHandle, state: &AppState) {
    let enabled = state.preferences.lock().await.notify_layer_switch;
    if !enabled {
        return;
    }
    let ids = state.engine.lock().await.layer_ids();
    let active_id = ids.into_iter().next().unwrap_or_default();
    let layer_name = {
        let reg = state.layer_registry.lock().await;
        reg.get(&active_id)
            .map(|p| p.name.clone())
            .unwrap_or(active_id)
    };
    crate::send_notification(app, "Layer Switched", &format!("Switched to {layer_name}"));
}

/// Send a profile-switch OS notification if `notify_profile_switch` is enabled.
pub(crate) async fn maybe_notify_profile_switch(app: &tauri::AppHandle, state: &AppState) {
    let enabled = state.preferences.lock().await.notify_profile_switch;
    if !enabled {
        return;
    }
    let ids = state.engine.lock().await.layer_ids();
    let active_id = ids.into_iter().next().unwrap_or_default();
    let profile_name = {
        let reg = state.layer_registry.lock().await;
        reg.get(&active_id)
            .map(|p| p.name.clone())
            .unwrap_or(active_id)
    };
    crate::send_notification(app, "Profile Switched", &format!("Switched to {profile_name}"));
}

// ── Haptic helpers ────────────────────────────────────────────────────────────

/// Built-in patterns from `docs/spec/haptics-spec.md` §Built-in patterns.
const PATTERN_SHORT_PULSE: &[u16] = &[80];
const PATTERN_DOUBLE_PULSE: &[u16] = &[80, 80, 80];
const PATTERN_TRIPLE_PULSE: &[u16] = &[80, 80, 80, 80, 80];

/// Send a pattern to all connected devices; no-op if BLE is unavailable.
async fn vibrate_pattern(state: &AppState, durations: &[u16]) {
    if let Some(ble) = &state.ble_manager {
        let pattern = VibrationPattern(durations.to_vec());
        ble.lock().await.vibrate_all(&pattern).await;
    }
}

/// Vibrate on tap resolved (short pulse), gated on `haptics_enabled` + `haptic_on_tap`.
pub(crate) async fn maybe_haptic_on_tap(state: &AppState) {
    let prefs = state.preferences.lock().await;
    if !prefs.haptics_enabled || !prefs.haptic_on_tap {
        return;
    }
    drop(prefs);
    vibrate_pattern(state, PATTERN_SHORT_PULSE).await;
}

/// Vibrate on layer switch (double pulse), gated on `haptics_enabled` + `haptic_on_layer_switch`.
pub(crate) async fn maybe_haptic_on_layer_switch(state: &AppState) {
    let prefs = state.preferences.lock().await;
    if !prefs.haptics_enabled || !prefs.haptic_on_layer_switch {
        return;
    }
    drop(prefs);
    vibrate_pattern(state, PATTERN_DOUBLE_PULSE).await;
}

/// Vibrate on profile switch (triple pulse), gated on `haptics_enabled` + `haptic_on_profile_switch`.
pub(crate) async fn maybe_haptic_on_profile_switch(state: &AppState) {
    let prefs = state.preferences.lock().await;
    if !prefs.haptics_enabled || !prefs.haptic_on_profile_switch {
        return;
    }
    drop(prefs);
    vibrate_pattern(state, PATTERN_TRIPLE_PULSE).await;
}

// ── Output processing ─────────────────────────────────────────────────────────

/// Process engine outputs without key simulation (used by command handlers).
pub async fn process_outputs_no_keys(app: &tauri::AppHandle, outputs: Vec<EngineOutput>) {
    for output in outputs {
        if let Some(debug) = output.debug {
            let _ = app.emit(DEBUG_EVENT, &debug);
        }
        for action in &output.actions {
            let _ = app.emit(
                ACTION_FIRED,
                ActionFiredPayload {
                    action_kind: action_kind_name(action).into(),
                    label: None,
                },
            );
        }
    }
}

/// Process engine outputs with full action dispatch (key simulation + layer ops).
pub async fn process_outputs(app: &tauri::AppHandle, state: &AppState, outputs: Vec<EngineOutput>) {
    for output in outputs {
        if let Some(debug) = output.debug {
            let _ = app.emit(DEBUG_EVENT, &debug);
        }
        for action in &output.actions {
            let _ = app.emit(
                ACTION_FIRED,
                ActionFiredPayload {
                    action_kind: action_kind_name(action).into(),
                    label: None,
                },
            );
            execute_action(app, state, action).await;
        }
        // The engine handled PopLayer internally and flagged that the stack
        // changed.  Emit here so the frontend stays in sync regardless of
        // whether there was an on_exit action to dispatch.
        if output.layer_changed {
            emit_layer_changed(app, state).await;
            maybe_notify_layer_switch(app, state).await;
            maybe_haptic_on_layer_switch(state).await;
        }
    }
}

// ── Action dispatch ───────────────────────────────────────────────────────────

/// Dispatch one `Action` to the platform.
async fn execute_action(app: &tauri::AppHandle, state: &AppState, action: &Action) {
    match action {
        Action::Key { key, modifiers } => {
            simulate_key(SimulatorCmd::Key {
                key: key.as_str().to_owned(),
                modifiers: modifiers.to_vec(),
            });
        }
        Action::KeyChord { keys } => {
            simulate_key(SimulatorCmd::Chord { keys: keys.clone() });
        }
        Action::TypeString { text } => {
            simulate_key(SimulatorCmd::TypeText { text: text.clone() });
        }
        Action::Macro { steps } => {
            // Execute steps inline. Key simulation calls are fire-and-forget
            // (`spawn_blocking`), so only the inter-step sleep delays hold the
            // pump; this is acceptable for typical short macro sequences.
            for step in steps {
                Box::pin(execute_action(app, state, &step.action)).await;
                if step.delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(step.delay_ms)).await;
                }
            }
        }
        Action::PushLayer { layer, mode } => {
            // LOCKING: acquires layer_registry then engine (in order).
            let profile = state.layer_registry.lock().await.get(layer).cloned();
            if let Some(profile) = profile {
                let outputs =
                    state
                        .engine
                        .lock()
                        .await
                        .push_layer(profile, mode.clone(), Instant::now());
                emit_layer_changed(app, state).await;
                maybe_notify_layer_switch(app, state).await;
                maybe_haptic_on_layer_switch(state).await;
                Box::pin(process_outputs(app, state, outputs)).await;
            } else {
                log::warn!("PushLayer: profile '{layer}' not found in registry");
            }
        }
        Action::PopLayer => {
            if let Some(outputs) = state.engine.lock().await.pop_layer() {
                emit_layer_changed(app, state).await;
                maybe_notify_layer_switch(app, state).await;
                maybe_haptic_on_layer_switch(state).await;
                Box::pin(process_outputs(app, state, outputs)).await;
            }
        }
        Action::SwitchLayer { layer } => {
            let profile = state.layer_registry.lock().await.get(layer).cloned();
            if let Some(profile) = profile {
                let outputs = state.engine.lock().await.switch_layer(profile);
                emit_layer_changed(app, state).await;
                maybe_notify_layer_switch(app, state).await;
                maybe_haptic_on_layer_switch(state).await;
                Box::pin(process_outputs(app, state, outputs)).await;
            } else {
                log::warn!("SwitchLayer: profile '{layer}' not found in registry");
            }
        }
        Action::ToggleVariable {
            variable,
            on_true,
            on_false,
        } => {
            // Read current value BEFORE flipping, select child, then flip.
            // LOCKING: acquires engine
            let (child, found) = {
                let mut engine = state.engine.lock().await;
                let current = engine.top_variables().get(variable.as_str()).cloned();
                let child = match &current {
                    Some(mapping_core::types::VariableValue::Bool(true)) => *on_true.clone(),
                    _ => *on_false.clone(),
                };
                engine.layer_stack_toggle_variable(variable);
                (child, current.is_some())
            };
            if !found {
                log::warn!("ToggleVariable: '{variable}' not found on top layer");
            }
            Box::pin(execute_action(app, state, &child)).await;
        }
        Action::SetVariable { variable, value } => {
            state
                .engine
                .lock()
                .await
                .layer_stack_set_variable(variable, value.clone());
        }
        Action::Conditional {
            variable,
            on_true,
            on_false,
        } => {
            let val = state
                .engine
                .lock()
                .await
                .top_variables()
                .get(variable.as_str())
                .cloned();
            let child = match val {
                Some(mapping_core::types::VariableValue::Bool(true)) => on_true,
                _ => on_false,
            };
            Box::pin(execute_action(app, state, child)).await;
        }
        Action::Block => {}
        // HoldModifier state is managed entirely inside ComboEngine before actions
        // reach the pump; by the time an action arrives here it has already had held
        // modifiers applied and the entry decremented/expired. Nothing to do.
        Action::HoldModifier { .. } => {}
        Action::MouseClick { button } => {
            simulate_key(SimulatorCmd::MouseClick { button: *button });
        }
        Action::MouseDoubleClick { button } => {
            simulate_key(SimulatorCmd::MouseDoubleClick { button: *button });
        }
        Action::MouseScroll { direction } => {
            simulate_key(SimulatorCmd::MouseScroll {
                direction: *direction,
            });
        }
        Action::Alias { name } => {
            let resolved = state.engine.lock().await.top_profile_alias(name);
            if let Some(a) = resolved {
                Box::pin(execute_action(app, state, &a)).await;
            } else {
                log::warn!("Alias '{name}' not found in current profile");
            }
        }
        Action::Vibrate { pattern } => {
            let enabled = state.preferences.lock().await.haptics_enabled;
            if enabled {
                if let Some(ble) = &state.ble_manager {
                    ble.lock().await.vibrate_all(pattern).await;
                }
            }
        }
    }
}

// ── Key simulation ────────────────────────────────────────────────────────────

/// Commands that can be sent to the blocking key-simulation thread.
#[derive(Debug, Clone)]
enum SimulatorCmd {
    Key {
        key: String,
        modifiers: Vec<Modifier>,
    },
    Chord {
        keys: Vec<String>,
    },
    TypeText {
        text: String,
    },
    MouseClick {
        button: MouseButton,
    },
    MouseDoubleClick {
        button: MouseButton,
    },
    MouseScroll {
        direction: ScrollDirection,
    },
}

/// Dispatch a keyboard/mouse simulation command on a blocking thread.
///
/// `Enigo` may be `!Send` on some platforms (e.g. X11). By creating and using
/// it entirely inside `spawn_blocking`, we keep the async runtime free of
/// non-Send types.
fn simulate_key(cmd: SimulatorCmd) {
    tokio::task::spawn_blocking(move || {
        use enigo::{Enigo, Keyboard as _, Mouse as _, Settings};

        let mut enigo = match Enigo::new(&Settings::default()) {
            Ok(e) => e,
            Err(err) => {
                log::warn!("key simulation: failed to init enigo: {err}");
                return;
            }
        };

        match cmd {
            SimulatorCmd::Key { key, modifiers } => {
                for m in &modifiers {
                    let _ = enigo.key(modifier_to_enigo(m), enigo::Direction::Press);
                }
                if let Some(k) = name_to_key(&key) {
                    let _ = enigo.key(k, enigo::Direction::Click);
                }
                for m in modifiers.iter().rev() {
                    let _ = enigo.key(modifier_to_enigo(m), enigo::Direction::Release);
                }
            }
            SimulatorCmd::Chord { keys } => {
                for k in &keys {
                    if let Some(ek) = name_to_key(k) {
                        let _ = enigo.key(ek, enigo::Direction::Click);
                    }
                }
            }
            SimulatorCmd::TypeText { text } => {
                if let Err(e) = enigo.text(&text) {
                    log::warn!("enigo text failed: {e}");
                }
            }
            SimulatorCmd::MouseClick { button } => {
                let btn = mouse_button_to_enigo(button);
                if let Err(e) = enigo.button(btn, enigo::Direction::Click) {
                    log::warn!("enigo mouse click failed: {e}");
                }
            }
            SimulatorCmd::MouseDoubleClick { button } => {
                let btn = mouse_button_to_enigo(button);
                for _ in 0..2 {
                    if let Err(e) = enigo.button(btn, enigo::Direction::Click) {
                        log::warn!("enigo mouse double-click failed: {e}");
                        break;
                    }
                }
            }
            SimulatorCmd::MouseScroll { direction } => {
                let (length, axis) = scroll_direction_to_enigo(direction);
                if let Err(e) = enigo.scroll(length, axis) {
                    log::warn!("enigo mouse scroll failed: {e}");
                }
            }
        }
    });
}

// ── Layer-changed helper ──────────────────────────────────────────────────────

/// Emit `layer-changed` with the current engine stack and update the tray.
// LOCKING: acquires engine, ble_manager
pub async fn emit_layer_changed(app: &tauri::AppHandle, state: &AppState) {
    let ids = state.engine.lock().await.layer_ids();
    let active = ids.first().cloned().unwrap_or_default();
    let stack: Vec<String> = ids.into_iter().rev().collect();
    let _ = app.emit(
        LAYER_CHANGED,
        LayerChangedPayload {
            stack,
            active: active.clone(),
        },
    );
    update_tray_from_state(app, state).await;
}

/// Refresh the tray tooltip and menu label from the current engine/BLE state.
// LOCKING: acquires engine, ble_manager
async fn update_tray_from_state(app: &tauri::AppHandle, state: &AppState) {
    // Resolve the active profile name from the registry (fall back to layer id).
    let active_layer_id = {
        let ids = state.engine.lock().await.layer_ids();
        ids.into_iter().next().unwrap_or_default()
    };
    let profile_name = {
        let reg = state.layer_registry.lock().await;
        reg.get(&active_layer_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "No profile active".to_string())
    };
    let device_count = state
        .ble_manager
        .as_ref()
        .map(|m| {
            // Use try_lock to avoid blocking; if the lock is contended just show 0.
            m.try_lock().map(|m| m.connected_ids().count()).unwrap_or(0)
        })
        .unwrap_or(0);
    crate::update_tray(app, &profile_name, device_count);
}

// ── Key name mapping ──────────────────────────────────────────────────────────

fn modifier_to_enigo(m: &Modifier) -> enigo::Key {
    match m {
        Modifier::Ctrl => enigo::Key::Control,
        Modifier::Shift => enigo::Key::Shift,
        Modifier::Alt => enigo::Key::Alt,
        Modifier::Meta => enigo::Key::Meta,
    }
}

fn mouse_button_to_enigo(button: MouseButton) -> enigo::Button {
    match button {
        MouseButton::Left => enigo::Button::Left,
        MouseButton::Right => enigo::Button::Right,
        MouseButton::Middle => enigo::Button::Middle,
    }
}

/// Returns `(length, axis)` for `enigo::Mouse::scroll`.
///
/// Positive length scrolls down (Vertical) or right (Horizontal).
fn scroll_direction_to_enigo(direction: ScrollDirection) -> (i32, enigo::Axis) {
    match direction {
        ScrollDirection::Up => (-1, enigo::Axis::Vertical),
        ScrollDirection::Down => (1, enigo::Axis::Vertical),
        ScrollDirection::Left => (-1, enigo::Axis::Horizontal),
        ScrollDirection::Right => (1, enigo::Axis::Horizontal),
    }
}

/// Map a key name string to an `enigo::Key`.
///
/// Returns `None` for keys that are not supported on the current platform; the caller
/// logs a warning and skips dispatch. See `docs/spec/extended-keys-spec.md` §Platform
/// availability for the full matrix.
fn name_to_key(name: &str) -> Option<enigo::Key> {
    use enigo::Key;
    match name {
        // ── Modifiers (used in KeyChord) ───────────────────────────────────────
        "ctrl" | "control" => Some(Key::Control),
        "shift" => Some(Key::Shift),
        "alt" => Some(Key::Alt),
        "meta" | "super" | "cmd" => Some(Key::Meta),

        // ── Letters ───────────────────────────────────────────────────────────
        "a" => Some(Key::Unicode('a')),
        "b" => Some(Key::Unicode('b')),
        "c" => Some(Key::Unicode('c')),
        "d" => Some(Key::Unicode('d')),
        "e" => Some(Key::Unicode('e')),
        "f" => Some(Key::Unicode('f')),
        "g" => Some(Key::Unicode('g')),
        "h" => Some(Key::Unicode('h')),
        "i" => Some(Key::Unicode('i')),
        "j" => Some(Key::Unicode('j')),
        "k" => Some(Key::Unicode('k')),
        "l" => Some(Key::Unicode('l')),
        "m" => Some(Key::Unicode('m')),
        "n" => Some(Key::Unicode('n')),
        "o" => Some(Key::Unicode('o')),
        "p" => Some(Key::Unicode('p')),
        "q" => Some(Key::Unicode('q')),
        "r" => Some(Key::Unicode('r')),
        "s" => Some(Key::Unicode('s')),
        "t" => Some(Key::Unicode('t')),
        "u" => Some(Key::Unicode('u')),
        "v" => Some(Key::Unicode('v')),
        "w" => Some(Key::Unicode('w')),
        "x" => Some(Key::Unicode('x')),
        "y" => Some(Key::Unicode('y')),
        "z" => Some(Key::Unicode('z')),

        // ── Digits ────────────────────────────────────────────────────────────
        "0" => Some(Key::Unicode('0')),
        "1" => Some(Key::Unicode('1')),
        "2" => Some(Key::Unicode('2')),
        "3" => Some(Key::Unicode('3')),
        "4" => Some(Key::Unicode('4')),
        "5" => Some(Key::Unicode('5')),
        "6" => Some(Key::Unicode('6')),
        "7" => Some(Key::Unicode('7')),
        "8" => Some(Key::Unicode('8')),
        "9" => Some(Key::Unicode('9')),

        // ── Punctuation (unshifted character via Unicode) ─────────────────────
        "grave" => Some(Key::Unicode('`')),
        "minus" => Some(Key::Unicode('-')),
        "equals" => Some(Key::Unicode('=')),
        "left_bracket" => Some(Key::Unicode('[')),
        "right_bracket" => Some(Key::Unicode(']')),
        "backslash" => Some(Key::Unicode('\\')),
        "semicolon" => Some(Key::Unicode(';')),
        "quote" => Some(Key::Unicode('\'')),
        "comma" => Some(Key::Unicode(',')),
        "period" => Some(Key::Unicode('.')),
        "slash" => Some(Key::Unicode('/')),

        // ── Navigation (cross-platform) ───────────────────────────────────────
        "space" => Some(Key::Space),
        "return" => Some(Key::Return),
        "backspace" => Some(Key::Backspace),
        "tab" => Some(Key::Tab),
        "escape" => Some(Key::Escape),
        "delete" => Some(Key::Delete),
        "home" => Some(Key::Home),
        "end" => Some(Key::End),
        "page_up" => Some(Key::PageUp),
        "page_down" => Some(Key::PageDown),
        "left_arrow" => Some(Key::LeftArrow),
        "right_arrow" => Some(Key::RightArrow),
        "up_arrow" => Some(Key::UpArrow),
        "down_arrow" => Some(Key::DownArrow),
        "caps_lock" => Some(Key::CapsLock),

        // ── Function keys (cross-platform F1–F20) ─────────────────────────────
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),
        "f13" => Some(Key::F13),
        "f14" => Some(Key::F14),
        "f15" => Some(Key::F15),
        "f16" => Some(Key::F16),
        "f17" => Some(Key::F17),
        "f18" => Some(Key::F18),
        "f19" => Some(Key::F19),
        "f20" => Some(Key::F20),

        // ── Media (cross-platform) ────────────────────────────────────────────
        "media_play" => Some(Key::MediaPlayPause),
        "media_next" => Some(Key::MediaNextTrack),
        "media_prev" => Some(Key::MediaPrevTrack),

        // ── Volume (cross-platform) ───────────────────────────────────────────
        "volume_up" => Some(Key::VolumeUp),
        "volume_down" => Some(Key::VolumeDown),
        "volume_mute" => Some(Key::VolumeMute),

        // ── Navigation (Windows + Linux; not macOS) — fall through on macOS ───
        #[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
        "insert" => Some(Key::Insert),
        #[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
        "num_lock" => Some(Key::Numlock),
        #[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
        "print_screen" => Some(Key::Print),

        // ── Navigation (Linux only) — fall through on other platforms ──────────
        #[cfg(all(unix, not(target_os = "macos")))]
        "scroll_lock" => Some(Key::ScrollLock),

        // ── Function keys (Windows + Linux; not macOS) ─────────────────────────
        #[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
        "f21" => Some(Key::F21),
        #[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
        "f22" => Some(Key::F22),
        #[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
        "f23" => Some(Key::F23),
        #[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
        "f24" => Some(Key::F24),

        // ── Media (Windows + Linux; not macOS) ─────────────────────────────────
        #[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
        "media_stop" => Some(Key::MediaStop),

        // ── System (Windows + Linux; not macOS) ───────────────────────────────
        #[cfg(any(target_os = "windows", all(unix, not(target_os = "macos"))))]
        "pause" => Some(Key::Pause),

        // ── System (macOS only) — fall through on other platforms ─────────────
        #[cfg(target_os = "macos")]
        "brightness_down" => Some(Key::BrightnessDown),
        #[cfg(target_os = "macos")]
        "brightness_up" => Some(Key::BrightnessUp),
        #[cfg(target_os = "macos")]
        "eject" => Some(Key::Eject),

        // ── System (Linux only) — fall through on other platforms ─────────────
        #[cfg(all(unix, not(target_os = "macos")))]
        "mic_mute" => Some(Key::MicMute),

        other => {
            log::warn!("name_to_key: unmapped key '{other}'");
            None
        }
    }
}

fn action_kind_name(action: &Action) -> &'static str {
    match action {
        Action::Key { .. } => "key",
        Action::KeyChord { .. } => "key_chord",
        Action::TypeString { .. } => "type_string",
        Action::Macro { .. } => "macro",
        Action::PushLayer { .. } => "push_layer",
        Action::PopLayer => "pop_layer",
        Action::SwitchLayer { .. } => "switch_layer",
        Action::ToggleVariable { .. } => "toggle_variable",
        Action::SetVariable { .. } => "set_variable",
        Action::Block => "block",
        Action::Conditional { .. } => "conditional",
        Action::Alias { .. } => "alias",
        Action::HoldModifier { .. } => "hold_modifier",
        Action::MouseClick { .. } => "mouse_click",
        Action::MouseDoubleClick { .. } => "mouse_double_click",
        Action::MouseScroll { .. } => "mouse_scroll",
        Action::Vibrate { .. } => "vibrate",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── mouse_button_to_enigo ─────────────────────────────────────────────────
    //
    // These tests verify the mapping from our domain type to the enigo type.
    // They do not require a display or any async runtime.

    #[test]
    fn mouse_button_to_enigo_left_maps_correctly() {
        assert_eq!(
            mouse_button_to_enigo(MouseButton::Left),
            enigo::Button::Left
        );
    }

    #[test]
    fn mouse_button_to_enigo_right_maps_correctly() {
        assert_eq!(
            mouse_button_to_enigo(MouseButton::Right),
            enigo::Button::Right
        );
    }

    #[test]
    fn mouse_button_to_enigo_middle_maps_correctly() {
        assert_eq!(
            mouse_button_to_enigo(MouseButton::Middle),
            enigo::Button::Middle
        );
    }

    // ── scroll_direction_to_enigo ─────────────────────────────────────────────

    #[test]
    fn scroll_direction_to_enigo_up_is_negative_vertical() {
        assert_eq!(
            scroll_direction_to_enigo(ScrollDirection::Up),
            (-1, enigo::Axis::Vertical)
        );
    }

    #[test]
    fn scroll_direction_to_enigo_down_is_positive_vertical() {
        assert_eq!(
            scroll_direction_to_enigo(ScrollDirection::Down),
            (1, enigo::Axis::Vertical)
        );
    }

    #[test]
    fn scroll_direction_to_enigo_left_is_negative_horizontal() {
        assert_eq!(
            scroll_direction_to_enigo(ScrollDirection::Left),
            (-1, enigo::Axis::Horizontal)
        );
    }

    #[test]
    fn scroll_direction_to_enigo_right_is_positive_horizontal() {
        assert_eq!(
            scroll_direction_to_enigo(ScrollDirection::Right),
            (1, enigo::Axis::Horizontal)
        );
    }

    // ── Manual verification steps ─────────────────────────────────────────────
    //
    // Full end-to-end dispatch (Enigo::new → button/scroll call) requires a
    // live display server and cannot run in CI without a virtual framebuffer.
    //
    // To verify manually:
    //
    // 1. Build and run the desktop app: `cargo tauri dev`
    // 2. Open the profile editor and create mappings for each mouse action:
    //    - mouse_click    { button: "left" }   → tap a chord
    //    - mouse_click    { button: "right" }  → tap a chord
    //    - mouse_click    { button: "middle" } → tap a chord
    //    - mouse_double_click { button: "left" } → tap a chord
    //    - mouse_scroll   { direction: "up" }  → tap a chord
    //    - mouse_scroll   { direction: "down" } → tap a chord
    //    - mouse_scroll   { direction: "left" } → tap a chord
    //    - mouse_scroll   { direction: "right" } → tap a chord
    // 3. Activate the profile and tap each chord.
    // 4. Verify:
    //    - Left/right/middle click land on the expected target element.
    //    - Double-click opens a file (test on the desktop or file manager).
    //    - Scroll up/down moves a scrollable area; left/right scrolls horizontally.
    //    - The debug panel ("action-fired" events) shows the correct action_kind
    //      for each tap.
}
