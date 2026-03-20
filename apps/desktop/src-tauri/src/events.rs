use std::collections::HashMap;

use serde::Serialize;

// ── Event name constants ──────────────────────────────────────────────────────

pub const TAP_EVENT: &str = "tap-event";
pub const ACTION_FIRED: &str = "action-fired";
pub const LAYER_CHANGED: &str = "layer-changed";
pub const DEVICE_CONNECTED: &str = "device-connected";
pub const DEVICE_DISCONNECTED: &str = "device-disconnected";
pub const DEBUG_EVENT: &str = "debug-event";
pub const PROFILE_ERROR: &str = "profile-error";
pub const CONTEXT_RULE_MATCHED: &str = "context-rule-matched";
pub const UPDATE_AVAILABLE: &str = "update-available";
pub const UPDATE_DOWNLOAD_PROGRESS: &str = "update-download-progress";

// ── Payload types ─────────────────────────────────────────────────────────────

/// Payload for [`TAP_EVENT`].
///
/// Emitted on every raw tap notification received from a connected device.
#[derive(Serialize, Clone)]
pub struct TapEventPayload {
    pub device_id: String,
    pub tap_code: u8,
    /// Wall-clock receive time as milliseconds since the Unix epoch, for use
    /// with `new Date(received_at_ms)` in the frontend.
    pub received_at_ms: u64,
}

impl From<&mapping_core::engine::RawTapEvent> for TapEventPayload {
    fn from(e: &mapping_core::engine::RawTapEvent) -> Self {
        // `Instant` has no epoch relationship; use SystemTime for the JS-friendly value.
        let received_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            device_id: e.device_id.to_string(),
            tap_code: e.tap_code,
            received_at_ms,
        }
    }
}

/// Payload for [`ACTION_FIRED`].
///
/// Emitted for every action dispatched to the platform layer.
#[derive(Serialize, Clone)]
pub struct ActionFiredPayload {
    /// Variant name of the `Action` enum (e.g. `"key"`, `"type_string"`).
    pub action_kind: String,
    /// Mapping label, if available.  Currently always `None`; reserved for a
    /// future improvement once `EngineOutput` carries label metadata.
    pub label: Option<String>,
}

/// Payload for [`LAYER_CHANGED`].
///
/// Emitted whenever the layer stack changes (push, pop, switch, activate).
#[derive(Serialize, Clone)]
pub struct LayerChangedPayload {
    /// Layer IDs from bottom (base) to top (active).
    pub stack: Vec<String>,
    /// `layer_id` of the currently active (top) layer.
    pub active: String,
}

/// Payload for [`DEVICE_CONNECTED`] and [`DEVICE_DISCONNECTED`].
#[derive(Serialize, Clone)]
pub struct DeviceStatusPayload {
    /// Logical role: `"solo"`, `"left"`, or `"right"`.
    pub role: String,
    /// BLE hardware address in `"AA:BB:CC:DD:EE:FF"` format.
    pub address: String,
}

/// Payload for [`PROFILE_ERROR`].
///
/// Emitted for each profile file that fails to load after a registry reload.
#[derive(Serialize, Clone)]
pub struct ProfileErrorPayload {
    pub file_name: String,
    pub message: String,
}

/// Payload for [`CONTEXT_RULE_MATCHED`].
///
/// Emitted when the focus monitor fires a context rule and activates a profile.
#[derive(Serialize, Clone)]
pub struct ContextRuleMatchedPayload {
    /// Human-readable label of the matched rule.
    pub rule_name: String,
    /// `layer_id` of the profile that was activated.
    pub layer_id: String,
}

/// Payload for [`UPDATE_AVAILABLE`].
///
/// Emitted by the background update checker and by the tray "Check for updates" action.
#[derive(Serialize, Clone)]
pub struct UpdateAvailablePayload {
    /// The new version string (e.g. `"1.2.0"`).
    pub version: String,
    /// Markdown release notes from the update manifest, if present.
    pub release_notes: Option<String>,
}

/// Payload for [`UPDATE_DOWNLOAD_PROGRESS`].
///
/// Emitted periodically during `download_and_install_update` to allow the
/// frontend to render a progress bar.
#[derive(Serialize, Clone)]
pub struct UpdateProgressPayload {
    /// Total bytes downloaded so far across all chunks.
    pub downloaded: u64,
    /// Total download size in bytes, if the server sent a `Content-Length` header.
    pub total: Option<u64>,
}

/// A connected device as returned by `get_engine_state`.
#[derive(Serialize)]
pub struct ConnectedDeviceDto {
    /// Logical role: `"solo"`, `"left"`, or `"right"`.
    pub role: String,
    /// BLE hardware address in `"AA:BB:CC:DD:EE:FF"` format.
    pub address: String,
}

/// Snapshot of the engine state returned by `get_engine_state`.
#[derive(Serialize)]
pub struct EngineStateSnapshot {
    /// Layer IDs from bottom (base) to top (active).
    pub layer_stack: Vec<String>,
    /// `layer_id` of the currently active (top) layer.
    pub active_layer_id: String,
    /// Current variable values on the top layer.
    pub variables: HashMap<String, serde_json::Value>,
    /// Currently connected BLE devices with their roles and addresses.
    pub connected_devices: Vec<ConnectedDeviceDto>,
    /// Whether debug mode is currently enabled.
    pub debug_mode: bool,
}
