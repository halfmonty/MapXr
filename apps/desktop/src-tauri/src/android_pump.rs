/// Android event pump — drives the combo engine from tap events forwarded by
/// the Kotlin `BlePlugin` and emits resolved actions back to the WebView and
/// Kotlin `AccessibilityPlugin`.
///
/// # Architecture
///
/// ```text
/// Kotlin BlePlugin
///   │  trigger("tap-bytes-received", { address, bytes })
///   ↓
/// WebView JS shim  (apps/desktop/src/lib/android-bridge.ts)
///   │  invoke("process_tap_event", { address, bytes })
///   ↓
/// Rust process_tap_event command  (commands.rs)
///   │  sends TapEventMsg over mpsc channel
///   ↓
/// run_android_pump (this file)
///   │  push_event / check_timeout → Vec<EngineOutput>
///   ↓
/// app.emit("tap-actions-fired", ...)   → WebView + Kotlin AccessibilityPlugin
/// app.emit("layer-changed", ...)       → WebView UI
/// app.emit("tap-vibrate", ...)         → Kotlin BlePlugin haptics
/// ```
use std::sync::Arc;
use std::time::Instant;

use mapping_core::engine::{DeviceId, RawTapEvent};
use mapping_core::types::Action;
use serde::Serialize;
use tauri::Emitter as _;
use tokio::sync::mpsc;

use crate::{
    events::{LayerChangedPayload, LAYER_CHANGED},
    state::AppState,
};

// ── Message type ──────────────────────────────────────────────────────────────

/// A raw tap packet forwarded from the Kotlin BLE plugin.
pub struct TapEventMsg {
    /// MAC address of the source Tap device (used as `DeviceId` on Android).
    pub address: String,
    /// Raw notification bytes (`bytes[0]` = tap\_code; `bytes[1–2]` = interval\_ms,
    /// unused by the engine).
    pub bytes: Vec<u8>,
}

// ── Event payloads ────────────────────────────────────────────────────────────

/// Payload for the `tap-actions-fired` Tauri event.
///
/// Consumed by WebView JS and by the Kotlin `AccessibilityPlugin` which listens
/// for this event and dispatches the actions as Android key/gesture events.
#[derive(Serialize, Clone)]
pub struct TapActionsFiredPayload {
    /// Serialised actions from the combo engine, in dispatch order.
    pub actions: Vec<Action>,
}

/// Payload for the `tap-vibrate` Tauri event.
///
/// Consumed by the Kotlin `BlePlugin`, which forwards the pattern to the
/// connected Tap device's vibration characteristic.
#[derive(Serialize, Clone)]
pub struct TapVibratePayload {
    /// MAC address of the target Tap device.
    pub address: String,
    /// Vibration on/off durations in milliseconds (alternating, starting with "on").
    pub pattern: Vec<u16>,
}

/// Name of the `tap-actions-fired` Tauri event.
pub(crate) const TAP_ACTIONS_FIRED: &str = "tap-actions-fired";
/// Name of the `tap-vibrate` Tauri event.
pub(crate) const TAP_VIBRATE: &str = "tap-vibrate";

// ── Pump ──────────────────────────────────────────────────────────────────────

/// Long-running async task that drives the combo engine on Android.
///
/// Mirrors the structure of the desktop `run_event_pump` in `pump.rs`, adapted
/// for Android:
/// - Tap events arrive over an `mpsc` channel from the `process_tap_event` command.
/// - Resolved actions are emitted as `tap-actions-fired` Tauri events rather than
///   dispatched via `enigo` — the Kotlin `AccessibilityPlugin` handles injection.
/// - Layer/profile switches are still handled in Rust; engine state is authoritative.
/// - `Vibrate` actions are relayed to Kotlin via `tap-vibrate` events.
pub async fn run_android_pump(
    app: tauri::AppHandle,
    state: Arc<AppState>,
    mut rx: mpsc::Receiver<TapEventMsg>,
) {
    loop {
        let next_deadline = state.engine.lock().await.next_deadline();

        let timeout_fut = async {
            match next_deadline {
                Some(d) => tokio::time::sleep_until(d.into()).await,
                None => std::future::pending::<()>().await,
            }
        };

        tokio::select! {
            msg = rx.recv() => {
                let Some(TapEventMsg { address, bytes }) = msg else {
                    // Channel closed — app is shutting down.
                    break;
                };

                let Some(&tap_code) = bytes.first() else {
                    continue; // empty packet
                };

                // The engine matches events by role ("solo"/"left"/"right"), not by
                // MAC address. Look up the persisted role for this device.
                let role = state
                    .android_devices
                    .lock()
                    .await
                    .get(&address)
                    .map(|r| r.role.clone());
                let Some(role) = role else {
                    log::warn!("process_tap_event: no role for {address} — ignoring");
                    continue;
                };

                let event = RawTapEvent {
                    device_id: DeviceId::new(&role),
                    tap_code,
                    received_at: Instant::now(),
                };

                let _ = app.emit(
                    crate::events::TAP_EVENT,
                    crate::events::TapEventPayload::from(&event),
                );

                let outputs = {
                    let mut engine = state.engine.lock().await;
                    engine.push_event(event, Instant::now())
                };

                process_android_outputs(&app, &state, outputs).await;
            }
            _ = timeout_fut => {
                let outputs = {
                    let mut engine = state.engine.lock().await;
                    engine.check_timeout(Instant::now())
                };
                process_android_outputs(&app, &state, outputs).await;
            }
        }
    }
}

// ── Output processing ─────────────────────────────────────────────────────────

pub(crate) async fn process_android_outputs(
    app: &tauri::AppHandle,
    state: &AppState,
    outputs: Vec<mapping_core::engine::EngineOutput>,
) {
    if outputs.is_empty() {
        return;
    }

    let mut dispatch_actions: Vec<Action> = Vec::new();

    for output in outputs {
        if let Some(debug) = output.debug {
            let _ = app.emit(crate::events::DEBUG_EVENT, &debug);
        }

        for action in &output.actions {
            match action {
                Action::PushLayer { layer, mode } => {
                    let profile = state.layer_registry.lock().await.get(layer).cloned();
                    if let Some(profile) = profile {
                        let sub = state
                            .engine
                            .lock()
                            .await
                            .push_layer(profile, mode.clone(), Instant::now());
                        emit_layer_changed(app, state).await;
                        Box::pin(process_android_outputs(app, state, sub)).await;
                    } else {
                        log::warn!("Android PushLayer: profile '{layer}' not found");
                    }
                }
                Action::PopLayer => {
                    if let Some(sub) = state.engine.lock().await.pop_layer() {
                        emit_layer_changed(app, state).await;
                        Box::pin(process_android_outputs(app, state, sub)).await;
                    }
                }
                Action::SwitchLayer { layer } => {
                    let profile = state.layer_registry.lock().await.get(layer).cloned();
                    if let Some(profile) = profile {
                        let sub = state.engine.lock().await.switch_layer(profile);
                        emit_layer_changed(app, state).await;
                        Box::pin(process_android_outputs(app, state, sub)).await;
                    } else {
                        log::warn!("Android SwitchLayer: profile '{layer}' not found");
                    }
                }
                Action::ToggleVariable { variable, on_true, on_false } => {
                    let (child, found) = {
                        let mut engine = state.engine.lock().await;
                        let current = engine.top_variables().get(variable.as_str()).cloned();
                        let child = match &current {
                            Some(mapping_core::types::VariableValue::Bool(true)) => {
                                *on_true.clone()
                            }
                            _ => *on_false.clone(),
                        };
                        engine.layer_stack_toggle_variable(variable);
                        (child, current.is_some())
                    };
                    if !found {
                        log::warn!("Android ToggleVariable: '{variable}' not found");
                    }
                    dispatch_actions.push(child);
                }
                Action::SetVariable { variable, value } => {
                    state
                        .engine
                        .lock()
                        .await
                        .layer_stack_set_variable(variable, value.clone());
                }
                Action::Conditional { variable, on_true, on_false } => {
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
                    dispatch_actions.push(*child.clone());
                }
                Action::Alias { name } => {
                    let resolved = state.engine.lock().await.top_profile_alias(name);
                    if let Some(a) = resolved {
                        dispatch_actions.push(a);
                    } else {
                        log::warn!("Android Alias '{name}' not found");
                    }
                }
                Action::Block | Action::HoldModifier { .. } => {}
                // All remaining actions (Key, KeyChord, TypeString, MouseClick,
                // MouseDoubleClick, MouseScroll, Macro, Vibrate) are forwarded to
                // Kotlin for dispatch via AccessibilityPlugin / BlePlugin.
                _ => dispatch_actions.push(action.clone()),
            }
        }

        if output.layer_changed {
            emit_layer_changed(app, state).await;
        }
    }

    if !dispatch_actions.is_empty() {
        let _ = app.emit(
            TAP_ACTIONS_FIRED,
            TapActionsFiredPayload { actions: dispatch_actions },
        );
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Emit a `layer-changed` event so the Svelte UI stays in sync with the engine.
pub(crate) async fn emit_layer_changed(app: &tauri::AppHandle, state: &AppState) {
    let ids = state.engine.lock().await.layer_ids();
    let active = ids.first().cloned().unwrap_or_default();
    let stack: Vec<String> = ids.into_iter().rev().collect();
    let _ = app.emit(LAYER_CHANGED, LayerChangedPayload { stack, active });
}
