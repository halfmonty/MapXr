use std::sync::Arc;
use std::time::Instant;

use mapping_core::engine::{EngineOutput, RawTapEvent};
use mapping_core::types::{Action, Modifier};
use tauri::Emitter as _;
use tokio::sync::broadcast;

use crate::{
    events::{
        ActionFiredPayload, LayerChangedPayload, TapEventPayload, ACTION_FIRED, DEBUG_EVENT,
        LAYER_CHANGED, TAP_EVENT,
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
    while let Ok(event) = status_rx.recv().await {
        let (event_name, payload) = ble_status_to_event(&event);
        let _ = app.emit(event_name, payload);
    }
}

fn ble_status_to_event(
    event: &tap_ble::BleStatusEvent,
) -> (&'static str, crate::events::DeviceStatusPayload) {
    match event {
        tap_ble::BleStatusEvent::Connected { device_id, address } => (
            crate::events::DEVICE_CONNECTED,
            crate::events::DeviceStatusPayload {
                role: device_id.to_string(),
                address: address.to_string(),
            },
        ),
        tap_ble::BleStatusEvent::Disconnected { device_id, address } => (
            crate::events::DEVICE_DISCONNECTED,
            crate::events::DeviceStatusPayload {
                role: device_id.to_string(),
                address: address.to_string(),
            },
        ),
    }
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
                Box::pin(process_outputs(app, state, outputs)).await;
            } else {
                log::warn!("PushLayer: profile '{layer}' not found in registry");
            }
        }
        Action::PopLayer => {
            let outputs = state.engine.lock().await.pop_layer();
            emit_layer_changed(app, state).await;
            Box::pin(process_outputs(app, state, outputs)).await;
        }
        Action::SwitchLayer { layer } => {
            let profile = state.layer_registry.lock().await.get(layer).cloned();
            if let Some(profile) = profile {
                let outputs = state.engine.lock().await.switch_layer(profile);
                emit_layer_changed(app, state).await;
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
        Action::Alias { name } => {
            let resolved = state.engine.lock().await.top_profile_alias(name);
            if let Some(a) = resolved {
                Box::pin(execute_action(app, state, &a)).await;
            } else {
                log::warn!("Alias '{name}' not found in current profile");
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
}

/// Dispatch a keyboard/mouse simulation command on a blocking thread.
///
/// `Enigo` may be `!Send` on some platforms (e.g. X11). By creating and using
/// it entirely inside `spawn_blocking`, we keep the async runtime free of
/// non-Send types.
fn simulate_key(cmd: SimulatorCmd) {
    tokio::task::spawn_blocking(move || {
        use enigo::{Enigo, Keyboard as _, Settings};

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
        }
    });
}

// ── Layer-changed helper ──────────────────────────────────────────────────────

/// Emit `layer-changed` with the current engine stack.
// LOCKING: acquires engine
pub async fn emit_layer_changed(app: &tauri::AppHandle, state: &AppState) {
    let ids = state.engine.lock().await.layer_ids();
    let active = ids.first().cloned().unwrap_or_default();
    let stack: Vec<String> = ids.into_iter().rev().collect();
    let _ = app.emit(LAYER_CHANGED, LayerChangedPayload { stack, active });
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

/// Map a key name string to an `enigo::Key`.
fn name_to_key(name: &str) -> Option<enigo::Key> {
    use enigo::Key;
    match name {
        "ctrl" | "control" | "Ctrl" | "Control" => Some(Key::Control),
        "shift" | "Shift" => Some(Key::Shift),
        "alt" | "Alt" => Some(Key::Alt),
        "meta" | "Meta" | "super" | "Super" | "cmd" | "Cmd" => Some(Key::Meta),
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
        "space" | "Space" => Some(Key::Space),
        "return" | "Return" | "enter" | "Enter" => Some(Key::Return),
        "backspace" | "BackSpace" => Some(Key::Backspace),
        "tab" | "Tab" => Some(Key::Tab),
        "escape" | "Escape" => Some(Key::Escape),
        "delete" | "Delete" => Some(Key::Delete),
        "home" | "Home" => Some(Key::Home),
        "end" | "End" => Some(Key::End),
        "page_up" | "Page_Up" => Some(Key::PageUp),
        "page_down" | "Page_Down" => Some(Key::PageDown),
        "left" | "Left" => Some(Key::LeftArrow),
        "right" | "Right" => Some(Key::RightArrow),
        "up" | "Up" => Some(Key::UpArrow),
        "down" | "Down" => Some(Key::DownArrow),
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
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
    }
}
