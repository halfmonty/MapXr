/// Android event pump — drives the combo engine from tap events and dispatches
/// resolved actions via Shizuku (bypasses WebView when backgrounded).
///
/// # Architecture (after Epic 21)
///
/// ```text
/// Kotlin BlePlugin.onTapBytes()
///   │  NativeBridge.processTapBytes(address, bytes)     ← JNI, always runs
///   ↓
/// run_android_pump (this file) via mpsc channel
///   │  push_event / check_timeout → Vec<EngineOutput>
///   ↓
/// dispatch_via_shizuku(actionsJson)                     ← JNI, always runs
///   │  ShizukuDispatcher.dispatch(actionsJson)          [Kotlin object, non-external]
///   ↓
/// IInputService.injectKey / injectMotion (shell uid via Shizuku)
///   ↓
/// InputManager.injectInputEvent()
///
/// WebView path (best-effort when foregrounded, UI only):
///   app.emit("tap-actions-fired", ...)   → debug panel event log
///   app.emit("layer-changed", ...)       → layer indicator
///   app.emit("tap-vibrate", ...)         → BlePlugin haptics
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
/// Consumed by WebView JS for the debug panel event log.
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
// TODO: task 15.10 — wire Vibrate action dispatch through BlePlugin tap-vibrate event
#[allow(dead_code)]
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
        // Serialise once; shared by the JNI callback and the WebView event.
        let actions_json = serde_json::to_string(&dispatch_actions).unwrap_or_else(|e| {
            log::warn!("android_pump: failed to serialise actions: {e}");
            String::from("[]")
        });

        // Native dispatch path — routes to ShizukuDispatcher via JNI, bypasses
        // WebView, works when app is backgrounded.
        dispatch_via_shizuku(&actions_json);

        // WebView event — keeps the debug panel populated when foregrounded.
        let _ = app.emit(
            TAP_ACTIONS_FIRED,
            TapActionsFiredPayload { actions: dispatch_actions },
        );
    }
}

// ── Shizuku dispatch ──────────────────────────────────────────────────────────

/// Call `ShizukuDispatcher.dispatch(actionsJson)` via JNI on the current thread.
///
/// `ShizukuDispatcher.dispatch` is a `@JvmStatic` Kotlin method on the singleton object
/// that converts the action JSON into `IInputService.injectKey` / `injectMotion` calls
/// running as shell uid via Shizuku.
///
/// Uses the [`JavaVM`] and `ShizukuDispatcher` class global ref stored by
/// `NativeBridge.registerShizukuDispatcher()`. If that has not yet been called
/// (very early startup), logs a warning and returns — the WebView `tap-actions-fired`
/// event is still emitted and the debug panel remains functional.
///
/// [`JavaVM`]: jni::JavaVM
#[cfg(target_os = "android")]
fn dispatch_via_shizuku(actions_json: &str) {
    use jni::objects::JValue;

    let (vm, class_ref) = match (
        crate::android_jni::java_vm(),
        crate::android_jni::shizuku_dispatcher_class(),
    ) {
        (Some(vm), Some(c)) => (vm, c),
        _ => {
            log::warn!(
                "android_pump: Shizuku dispatch not registered — actions delivered via WebView only"
            );
            return;
        }
    };

    if let Err(e) = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let j_str = env.new_string(actions_json)?;
        env.call_static_method(
            class_ref,
            jni::jni_str!("dispatch"),
            jni::jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(j_str.as_ref())],
        )?;
        Ok(())
    }) {
        eprintln!("mapxr/pump: ShizukuDispatcher.dispatch() failed: {e}");
    }
}

/// No-op on non-Android mobile targets (iOS).
#[cfg(not(target_os = "android"))]
fn dispatch_via_shizuku(_actions_json: &str) {}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Emit a `layer-changed` event so the Svelte UI stays in sync with the engine.
pub(crate) async fn emit_layer_changed(app: &tauri::AppHandle, state: &AppState) {
    let ids = state.engine.lock().await.layer_ids();
    let active = ids.first().cloned().unwrap_or_default();
    let stack: Vec<String> = ids.into_iter().rev().collect();
    let _ = app.emit(LAYER_CHANGED, LayerChangedPayload { stack, active });
}
