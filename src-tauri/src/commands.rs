use std::str::FromStr as _;

use btleplug::api::BDAddr;
use mapping_core::{
    engine::DeviceId,
    types::{Profile, PushLayerMode},
};
use serde::Serialize;
use std::sync::Arc;

use tauri::State;

use tauri::Emitter as _;

use crate::{events::{ConnectedDeviceDto, EngineStateSnapshot}, state::AppState};

// ── DTOs ──────────────────────────────────────────────────────────────────────

/// Lightweight serialisable summary of a discovered BLE device.
#[derive(Serialize)]
pub struct TapDeviceInfoDto {
    pub name: Option<String>,
    /// `"AA:BB:CC:DD:EE:FF"` format.
    pub address: String,
    pub rssi: Option<i16>,
}

impl From<&tap_ble::TapDeviceInfo> for TapDeviceInfoDto {
    fn from(d: &tap_ble::TapDeviceInfo) -> Self {
        Self {
            name: d.name.clone(),
            address: d.address.to_string(),
            rssi: d.rssi,
        }
    }
}

/// Lightweight serialisable summary of a profile (for list views).
#[derive(Serialize)]
pub struct ProfileSummary {
    pub layer_id: String,
    pub name: String,
    /// `"single"` or `"dual"`.
    pub kind: String,
}

impl From<&Profile> for ProfileSummary {
    fn from(p: &Profile) -> Self {
        Self {
            layer_id: p.layer_id.clone(),
            name: p.name.clone(),
            kind: match p.kind {
                mapping_core::types::ProfileKind::Single => "single".into(),
                mapping_core::types::ProfileKind::Dual => "dual".into(),
            },
        }
    }
}

// ── Helper: emit layer-changed ────────────────────────────────────────────────

// emit_layer_changed is provided by crate::pump::emit_layer_changed

// ── 4.1 scan_devices ─────────────────────────────────────────────────────────

/// Scan for nearby Tap devices for 5 seconds.
///
/// Returns discovered devices sorted by signal strength (strongest first).
#[tauri::command]
pub async fn scan_devices(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<TapDeviceInfoDto>, String> {
    state.require_ble()?;
    // LOCKING: acquires ble_manager
    let ble = state.ble_manager.as_ref().unwrap();
    let devices = ble
        .lock()
        .await
        .scan(5000)
        .await
        .map_err(|e| e.to_string())?;
    Ok(devices.iter().map(TapDeviceInfoDto::from).collect())
}

// ── 4.2 connect_device ───────────────────────────────────────────────────────

/// Connect to the Tap device at `address` and assign it `role`.
///
/// `address` must be a colon-separated hex string (`"AA:BB:CC:DD:EE:FF"`).
/// `role` must be `"solo"`, `"left"`, or `"right"`.
///
/// On success the device is saved to `devices.json` so it reconnects
/// automatically in future sessions.
#[tauri::command]
pub async fn connect_device(
    state: State<'_, Arc<AppState>>,
    address: String,
    role: String,
) -> Result<(), String> {
    state.require_ble()?;

    let addr = BDAddr::from_str(&address).map_err(|_| format!("invalid BLE address: {address}"))?;

    if !matches!(role.as_str(), "solo" | "left" | "right") {
        return Err(format!(
            "invalid role '{role}': must be 'solo', 'left', or 'right'"
        ));
    }
    let device_id = DeviceId::new(&role);

    // LOCKING: acquires ble_manager
    state
        .ble_manager
        .as_ref()
        .unwrap()
        .lock()
        .await
        .connect(device_id.clone(), addr)
        .await
        .map_err(|e| e.to_string())?;

    // Persist the role → address mapping.
    {
        // LOCKING: acquires device_registry (std::sync::Mutex)
        let mut reg = state.device_registry.lock().map_err(|e| e.to_string())?;
        reg.assign(device_id, addr);
        reg.save(&state.devices_json_path)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ── 4.3 disconnect_device ────────────────────────────────────────────────────

/// Disconnect the device assigned to `role`.
///
/// Returns `Ok(())` if no device is connected under that role.
/// Also removes the device from the persistent registry so it is not
/// auto-reconnected on the next app launch.
#[tauri::command]
pub async fn disconnect_device(
    state: State<'_, Arc<AppState>>,
    role: String,
) -> Result<(), String> {
    state.require_ble()?;
    let device_id = DeviceId::new(&role);

    // LOCKING: acquires ble_manager
    state
        .ble_manager
        .as_ref()
        .unwrap()
        .lock()
        .await
        .disconnect(&device_id)
        .await
        .map_err(|e| e.to_string())?;

    // Remove from persistent registry so auto-reconnect skips this device.
    // LOCKING: acquires device_registry (std::sync::Mutex)
    {
        let mut reg = state.device_registry.lock().map_err(|e| e.to_string())?;
        reg.remove(&device_id);
        reg.save(&state.devices_json_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ── 4.4 list_profiles ────────────────────────────────────────────────────────

/// List all profiles in the profiles directory.
///
/// Triggers a reload so newly added files are included.  Any file that fails
/// to load is silently skipped here; a `profile-error` Tauri event is emitted
/// for each failure (wired up by the Tauri event layer).
#[tauri::command]
pub async fn list_profiles(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<Vec<ProfileSummary>, String> {
    // LOCKING: acquires layer_registry
    let mut reg = state.layer_registry.lock().await;
    let _ = reg.reload();

    // Emit profile-error for each failed file.
    for (path, err) in reg.load_errors() {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_owned();
        let _ = app.emit(
            crate::events::PROFILE_ERROR,
            crate::events::ProfileErrorPayload {
                file_name,
                message: err.to_string(),
            },
        );
    }

    let mut summaries: Vec<ProfileSummary> = reg.profiles().map(ProfileSummary::from).collect();
    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(summaries)
}

// ── 4.5 load_profile ─────────────────────────────────────────────────────────

/// Load the full profile with `layer_id`.
#[tauri::command]
pub async fn load_profile(
    state: State<'_, Arc<AppState>>,
    layer_id: String,
) -> Result<Profile, String> {
    // LOCKING: acquires layer_registry
    let reg = state.layer_registry.lock().await;
    reg.get(&layer_id)
        .cloned()
        .ok_or_else(|| format!("profile not found: {layer_id}"))
}

// ── 4.6 save_profile ─────────────────────────────────────────────────────────

/// Write `profile` to disk and reload the registry.
#[tauri::command]
pub async fn save_profile(state: State<'_, Arc<AppState>>, profile: Profile) -> Result<(), String> {
    // Validate before touching the filesystem so we never write an invalid profile.
    profile.validate().map_err(|e| e.to_string())?;

    let path = state
        .profiles_dir
        .join(format!("{}.json", profile.layer_id));
    profile.save(&path).map_err(|e| e.to_string())?;

    // LOCKING: acquires layer_registry
    let _ = state.layer_registry.lock().await.reload();
    Ok(())
}

// ── 4.7 delete_profile ───────────────────────────────────────────────────────

/// Delete the profile file for `layer_id`.
#[tauri::command]
pub async fn delete_profile(
    state: State<'_, Arc<AppState>>,
    layer_id: String,
) -> Result<(), String> {
    let path = state.profiles_dir.join(format!("{layer_id}.json"));
    std::fs::remove_file(&path)
        .map_err(|e| format!("could not delete profile '{layer_id}': {e}"))?;

    // LOCKING: acquires layer_registry
    let _ = state.layer_registry.lock().await.reload();
    Ok(())
}

// ── 4.8 activate_profile ─────────────────────────────────────────────────────

/// Replace the engine's base layer with the profile identified by `layer_id`.
///
/// Clears all pending state and emits `layer-changed`.
#[tauri::command]
pub async fn activate_profile(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    layer_id: String,
) -> Result<(), String> {
    // Look up profile without holding the engine lock.
    // LOCKING: acquires layer_registry
    let profile = {
        let reg = state.layer_registry.lock().await;
        reg.get(&layer_id)
            .cloned()
            .ok_or_else(|| format!("profile not found: {layer_id}"))?
    };

    // LOCKING: acquires engine
    state.engine.lock().await.set_profile(profile);

    crate::pump::emit_layer_changed(&app, &state).await;
    Ok(())
}

// ── deactivate_profile ───────────────────────────────────────────────────────

/// Reset the engine to the built-in default profile, leaving no user profile active.
///
/// After this call `get_engine_state` will return an `active_layer_id` of `"default"`,
/// which does not match any user profile in the registry, so the UI shows no profile
/// as active.
#[tauri::command]
pub async fn deactivate_profile(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // LOCKING: acquires engine
    state
        .engine
        .lock()
        .await
        .set_profile(crate::state::builtin_default_profile());

    crate::pump::emit_layer_changed(&app, &state).await;
    Ok(())
}

// ── 4.9 push_layer ───────────────────────────────────────────────────────────

/// Push the profile identified by `layer_id` onto the engine's layer stack.
#[tauri::command]
pub async fn push_layer(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    layer_id: String,
    mode: PushLayerMode,
) -> Result<(), String> {
    // Look up profile first, without holding the engine lock.
    // LOCKING: acquires layer_registry
    let profile = {
        let reg = state.layer_registry.lock().await;
        reg.get(&layer_id)
            .cloned()
            .ok_or_else(|| format!("profile not found: {layer_id}"))?
    };

    // LOCKING: acquires engine
    let outputs = state
        .engine
        .lock()
        .await
        .push_layer(profile, mode, std::time::Instant::now());

    crate::pump::process_outputs(&app, &state, outputs).await;

    crate::pump::emit_layer_changed(&app, &state).await;
    Ok(())
}

// ── 4.10 pop_layer ───────────────────────────────────────────────────────────

/// Pop the top layer off the engine's layer stack.
///
/// Returns an error if the stack is already at the base layer.
#[tauri::command]
pub async fn pop_layer(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // LOCKING: acquires engine
    let outputs = state.engine.lock().await.pop_layer();
    if outputs.is_empty() {
        return Err("Layer stack is at base layer; nothing to pop".into());
    }
    crate::pump::process_outputs(&app, &state, outputs).await;
    crate::pump::emit_layer_changed(&app, &state).await;
    Ok(())
}

// ── 4.11 set_debug_mode ──────────────────────────────────────────────────────

/// Enable or disable debug event emission.
#[tauri::command]
pub async fn set_debug_mode(state: State<'_, Arc<AppState>>, enabled: bool) -> Result<(), String> {
    // LOCKING: acquires engine
    state.engine.lock().await.set_debug(enabled);
    Ok(())
}

// ── 4.12 get_engine_state ────────────────────────────────────────────────────

/// Return a snapshot of the current engine state.
#[tauri::command]
pub async fn get_engine_state(
    state: State<'_, Arc<AppState>>,
) -> Result<EngineStateSnapshot, String> {
    // LOCKING: acquires engine
    let (layer_stack_top_first, variables_raw, debug_mode) = {
        let engine = state.engine.lock().await;
        (
            engine.layer_ids(),
            engine.top_variables().clone(),
            engine.debug_mode(),
        )
    };

    let active_layer_id = layer_stack_top_first.first().cloned().unwrap_or_default();
    let layer_stack: Vec<String> = layer_stack_top_first.into_iter().rev().collect();

    // Convert VariableValue to serde_json::Value for the frontend.
    let variables = variables_raw
        .into_iter()
        .map(|(k, v)| {
            let json_val = serde_json::to_value(&v).unwrap_or(serde_json::Value::Null);
            (k, json_val)
        })
        .collect();

    // LOCKING: acquires ble_manager
    let connected_devices = match &state.ble_manager {
        Some(ble) => ble
            .lock()
            .await
            .connected_devices()
            .map(|(id, addr)| ConnectedDeviceDto {
                role: id.to_string(),
                address: addr.to_string(),
            })
            .collect(),
        None => vec![],
    };

    Ok(EngineStateSnapshot {
        layer_stack,
        active_layer_id,
        variables,
        connected_devices,
        debug_mode,
    })
}

// ── read_file_text ────────────────────────────────────────────────────────────

/// Read a file at an absolute path and return its contents as a UTF-8 string.
///
/// Used by the frontend to read files dropped onto the window via drag-and-drop:
/// on Linux/WebKitGTK the WebView receives a `file://` URI rather than a `File`
/// object, so the frontend resolves the path and delegates the read to this command.
#[tauri::command]
pub fn read_file_text(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read '{path}': {e}"))
}

// ── DTO unit tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mapping_core::types::{Hand, ProfileKind, ProfileSettings};

    fn test_profile(layer_id: &str, name: &str, kind: ProfileKind) -> Profile {
        Profile {
            version: 1,
            kind,
            name: name.into(),
            description: None,
            layer_id: layer_id.into(),
            hand: Some(Hand::Right),
            passthrough: false,
            settings: ProfileSettings::default(),
            aliases: std::collections::HashMap::new(),
            variables: std::collections::HashMap::new(),
            on_enter: None,
            on_exit: None,
            mappings: vec![],
        }
    }

    #[test]
    fn profile_summary_from_single_profile() {
        let p = test_profile("base", "My Profile", ProfileKind::Single);
        let s = ProfileSummary::from(&p);
        assert_eq!(s.layer_id, "base");
        assert_eq!(s.name, "My Profile");
        assert_eq!(s.kind, "single");
    }

    #[test]
    fn profile_summary_from_dual_profile() {
        let p = test_profile("dual_layer", "Dual", ProfileKind::Dual);
        let s = ProfileSummary::from(&p);
        assert_eq!(s.kind, "dual");
    }

    #[test]
    fn tap_device_info_dto_from_device_info() {
        use std::str::FromStr as _;
        let info = tap_ble::TapDeviceInfo {
            name: Some("Tap Strap 2".into()),
            address: BDAddr::from_str("AA:BB:CC:DD:EE:FF").unwrap(),
            rssi: Some(-60),
        };
        let dto = TapDeviceInfoDto::from(&info);
        assert_eq!(dto.address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(dto.name.as_deref(), Some("Tap Strap 2"));
        assert_eq!(dto.rssi, Some(-60));
    }
}
