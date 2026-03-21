use mapping_core::types::{Profile, PushLayerMode};
use serde::Serialize;
use std::sync::Arc;

use tauri::State;
use tauri::Emitter as _;

use crate::{
    events::{ConnectedDeviceDto, EngineStateSnapshot},
    state::AppState,
};

#[cfg(not(mobile))]
use std::str::FromStr as _;
#[cfg(not(mobile))]
use btleplug::api::BDAddr;
#[cfg(not(mobile))]
use mapping_core::engine::DeviceId;

// ── DTOs ──────────────────────────────────────────────────────────────────────

/// Lightweight serialisable summary of a discovered BLE device (desktop only).
#[cfg(not(mobile))]
#[derive(Serialize)]
pub struct TapDeviceInfoDto {
    pub name: Option<String>,
    /// `"AA:BB:CC:DD:EE:FF"` format.
    pub address: String,
    pub rssi: Option<i16>,
    /// `true` if the device was actively advertising during the current scan window.
    /// `false` means the entry came from the OS Bluetooth cache and the device may be off.
    pub seen_in_scan: bool,
    /// `true` if the device currently has an active BLE connection to this host.
    /// The device's connection slot is occupied; our app cannot connect until it is released.
    pub is_connected_to_os: bool,
}

#[cfg(not(mobile))]
impl From<&tap_ble::TapDeviceInfo> for TapDeviceInfoDto {
    fn from(d: &tap_ble::TapDeviceInfo) -> Self {
        Self {
            name: d.name.clone(),
            address: d.address.to_string(),
            rssi: d.rssi,
            seen_in_scan: d.seen_in_scan,
            is_connected_to_os: d.is_connected_to_os,
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
#[cfg(not(mobile))]
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

#[cfg(not(mobile))]
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
#[cfg(not(mobile))]
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
        reg.save(&state.devices_json_path)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ── reassign_device_role ──────────────────────────────────────────────────────

/// Reassign the connected device at `address` to `new_role` without disconnecting.
///
/// `address` must be a colon-separated hex string (`"AA:BB:CC:DD:EE:FF"`).
/// `new_role` must be `"solo"`, `"left"`, or `"right"`.
///
/// Background tasks are restarted under the new role so subsequent tap events and
/// status notifications carry the correct identity. The device registry is updated
/// and persisted so auto-reconnect uses the new role on the next launch.
#[cfg(not(mobile))]
#[tauri::command]
pub async fn reassign_device_role(
    state: State<'_, Arc<AppState>>,
    address: String,
    new_role: String,
) -> Result<(), String> {
    state.require_ble()?;

    if !matches!(new_role.as_str(), "solo" | "left" | "right") {
        return Err(format!(
            "invalid role '{new_role}': must be 'solo', 'left', or 'right'"
        ));
    }

    let addr = BDAddr::from_str(&address).map_err(|_| format!("invalid BLE address: {address}"))?;
    let new_id = DeviceId::new(&new_role);

    // Find which role currently owns this address.
    // LOCKING: acquires ble_manager (guard dropped when block exits)
    let old_id = {
        let ble = state.ble_manager.as_ref().unwrap().lock().await;
        // Save to a named binding so the iterator (which borrows `ble`) is
        // fully evaluated and dropped before `ble` goes out of scope.
        let result = ble
            .connected_devices()
            .find(|(_, a)| *a == addr)
            .map(|(id, _)| id.clone())
            .ok_or_else(|| format!("no connected device at address {address}"));
        result
    }?;

    // LOCKING: acquires ble_manager
    state
        .ble_manager
        .as_ref()
        .unwrap()
        .lock()
        .await
        .reassign_role(&old_id, new_id.clone())
        .await
        .map_err(|e| e.to_string())?;

    // Persist the updated role → address mapping.
    // LOCKING: acquires device_registry (std::sync::Mutex)
    {
        let mut reg = state.device_registry.lock().map_err(|e| e.to_string())?;
        reg.assign(new_id, addr);
        reg.save(&state.devices_json_path)
            .map_err(|e| e.to_string())?;
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

/// Write `profile` to disk, reload the registry, and hot-reload the engine if
/// the saved profile is currently active.
///
/// If the profile's `layer_id` appears anywhere in the current engine layer
/// stack, `set_profile` is called with the freshly saved version so changes
/// take effect immediately without the user having to deactivate and reactivate.
/// Any layers pushed on top of the updated base are cleared (the stack is reset
/// to just the new version of the saved profile), which is the expected
/// behaviour when the base definition has changed.
#[tauri::command]
pub async fn save_profile(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    profile: Profile,
) -> Result<(), String> {
    // Validate before touching the filesystem so we never write an invalid profile.
    profile.validate().map_err(|e| e.to_string())?;

    let path = state
        .profiles_dir
        .join(format!("{}.json", profile.layer_id));
    profile.save(&path).map_err(|e| e.to_string())?;

    // LOCKING: acquires layer_registry
    let _ = state.layer_registry.lock().await.reload();

    // Hot-reload: if the saved profile is anywhere in the current engine stack,
    // replace it as the base layer so edits take effect immediately.
    // LOCKING: acquires engine (single lock acquisition for read + conditional write)
    let needs_reload = {
        let mut engine = state.engine.lock().await;
        if engine.layer_ids().iter().any(|id| id == &profile.layer_id) {
            engine.set_profile(profile);
            true
        } else {
            false
        }
    };
    if needs_reload {
        crate::pump::emit_layer_changed(&app, &state).await;
    }

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

    // Persist so this profile is restored on the next launch.
    {
        let mut prefs = state.preferences.lock().await;
        prefs.profile_active = true;
        prefs.last_active_profile_id = Some(layer_id);
        if let Err(e) = prefs.save(&state.preferences_path) {
            log::warn!("failed to save preferences: {e}");
        }
    }

    crate::pump::emit_layer_changed(&app, &state).await;
    crate::pump::maybe_notify_profile_switch(&app, &state).await;
    crate::pump::maybe_haptic_on_profile_switch(&state).await;
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

    // Clear the persisted profile so the next launch also starts with no active profile.
    {
        let mut prefs = state.preferences.lock().await;
        prefs.profile_active = false;
        prefs.last_active_profile_id = None;
        if let Err(e) = prefs.save(&state.preferences_path) {
            log::warn!("failed to save preferences: {e}");
        }
    }

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
    crate::pump::maybe_notify_layer_switch(&app, &state).await;
    crate::pump::maybe_haptic_on_layer_switch(&state).await;
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
    let Some(outputs) = state.engine.lock().await.pop_layer() else {
        return Err("Layer stack is at base layer; nothing to pop".into());
    };
    crate::pump::process_outputs(&app, &state, outputs).await;
    crate::pump::emit_layer_changed(&app, &state).await;
    crate::pump::maybe_notify_layer_switch(&app, &state).await;
    crate::pump::maybe_haptic_on_layer_switch(&state).await;
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

    // LOCKING: acquires ble_manager (desktop only; Android BLE state lives in Kotlin)
    #[cfg(not(mobile))]
    let connected_devices: Vec<ConnectedDeviceDto> = match &state.ble_manager {
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
    #[cfg(mobile)]
    let connected_devices: Vec<ConnectedDeviceDto> = vec![];

    Ok(EngineStateSnapshot {
        layer_stack,
        active_layer_id,
        variables,
        connected_devices,
        debug_mode,
    })
}

// ── 10.3 rename_device ───────────────────────────────────────────────────────

/// Validate a candidate device name against the rules in `docs/spec/device-rename-spec.md`.
///
/// Returns `Ok(trimmed_name)` on success or `Err(message)` describing the first violation.
#[cfg(not(mobile))]
fn validate_device_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim().to_owned();
    if trimmed.is_empty() {
        return Err("device name must not be empty".into());
    }
    if trimmed.len() > 20 {
        return Err(format!(
            "device name is too long ({} characters); maximum is 20",
            trimmed.len()
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii() && !c.is_ascii_control())
    {
        return Err("device name must contain only printable ASCII characters".into());
    }
    Ok(trimmed)
}

/// Rename a connected Tap device.
///
/// `address` — the BDAddr string identifying the device (e.g. `"AA:BB:CC:DD:EE:FF"`).
/// `name` — the desired new name (1–20 printable ASCII chars; leading/trailing whitespace
/// is trimmed automatically).
///
/// Returns `Ok(())` on success. Returns `Err(message)` if the device is not connected,
/// the name fails validation, or the BLE write fails.
#[cfg(not(mobile))]
#[tauri::command]
pub async fn rename_device(
    state: State<'_, Arc<AppState>>,
    address: String,
    name: String,
) -> Result<(), String> {
    state.require_ble()?;

    let addr = BDAddr::from_str(&address).map_err(|_| format!("invalid BLE address: {address}"))?;
    let validated_name = validate_device_name(&name)?;

    // LOCKING: acquires ble_manager
    state
        .ble_manager
        .as_ref()
        .unwrap()
        .lock()
        .await
        .set_device_name(addr, &validated_name)
        .await
        .map_err(|e| e.to_string())
}

// ── 11.5 list_context_rules / save_context_rules ─────────────────────────────

/// Return the current context rules.
#[cfg(not(mobile))]
#[tauri::command]
pub async fn list_context_rules(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::context_rules::ContextRules, String> {
    // LOCKING: acquires context_rules
    Ok(state.context_rules.lock().await.clone())
}

/// Validate, persist, and replace the context rules.
///
/// The new rules are validated before writing. On success the in-memory state
/// is updated atomically so the monitor immediately uses the new rules.
#[cfg(not(mobile))]
#[tauri::command]
pub async fn save_context_rules(
    state: State<'_, Arc<AppState>>,
    rules: crate::context_rules::ContextRules,
) -> Result<(), String> {
    rules.validate()?;
    rules.save(&state.context_rules_path)?;
    // LOCKING: acquires context_rules
    *state.context_rules.lock().await = rules;
    Ok(())
}

// ── 12.5 get_preferences / save_preferences (desktop only) ───────────────────
// Android preferences are handled by a separate command added in task 15.2.

/// All user preferences exposed to the frontend settings page.
#[cfg(not(mobile))]
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TrayPreferences {
    /// Hide window on close instead of quitting.
    pub close_to_tray: bool,
    /// Launch directly to tray without showing the main window.
    pub start_minimised: bool,
    /// Register the app to start automatically at OS login.
    pub start_at_login: bool,
    /// Notify when a Tap device connects.
    pub notify_device_connected: bool,
    /// Notify when a Tap device disconnects.
    pub notify_device_disconnected: bool,
    /// Notify when the active layer switches within a profile.
    pub notify_layer_switch: bool,
    /// Notify when the active profile switches.
    pub notify_profile_switch: bool,
    /// Master haptic toggle — gates all vibration.
    pub haptics_enabled: bool,
    /// Vibrate on every resolved tap event.
    pub haptic_on_tap: bool,
    /// Vibrate on layer push/pop/switch.
    pub haptic_on_layer_switch: bool,
    /// Vibrate on profile activate.
    pub haptic_on_profile_switch: bool,
}

/// Return the current preferences.
#[cfg(not(mobile))]
#[tauri::command]
pub async fn get_preferences(state: State<'_, Arc<AppState>>) -> Result<TrayPreferences, String> {
    let prefs = state.preferences.lock().await;
    Ok(TrayPreferences {
        close_to_tray: prefs.close_to_tray,
        start_minimised: prefs.start_minimised,
        start_at_login: prefs.start_at_login,
        notify_device_connected: prefs.notify_device_connected,
        notify_device_disconnected: prefs.notify_device_disconnected,
        notify_layer_switch: prefs.notify_layer_switch,
        notify_profile_switch: prefs.notify_profile_switch,
        haptics_enabled: prefs.haptics_enabled,
        haptic_on_tap: prefs.haptic_on_tap,
        haptic_on_layer_switch: prefs.haptic_on_layer_switch,
        haptic_on_profile_switch: prefs.haptic_on_profile_switch,
    })
}

/// Persist updated preferences and apply live effects.
///
/// `start_at_login` changes take effect immediately (registers/deregisters the
/// OS login item). Notification flags take effect immediately (checked at event
/// time). Other settings are read at the point they become relevant.
#[cfg(not(mobile))]
#[tauri::command]
pub async fn save_preferences(
    state: State<'_, Arc<AppState>>,
    prefs_update: TrayPreferences,
) -> Result<(), String> {
    let start_at_login_changed;

    {
        let mut prefs = state.preferences.lock().await;
        start_at_login_changed = prefs.start_at_login != prefs_update.start_at_login;
        prefs.close_to_tray = prefs_update.close_to_tray;
        prefs.start_minimised = prefs_update.start_minimised;
        prefs.start_at_login = prefs_update.start_at_login;
        prefs.notify_device_connected = prefs_update.notify_device_connected;
        prefs.notify_device_disconnected = prefs_update.notify_device_disconnected;
        prefs.notify_layer_switch = prefs_update.notify_layer_switch;
        prefs.notify_profile_switch = prefs_update.notify_profile_switch;
        prefs.haptics_enabled = prefs_update.haptics_enabled;
        prefs.haptic_on_tap = prefs_update.haptic_on_tap;
        prefs.haptic_on_layer_switch = prefs_update.haptic_on_layer_switch;
        prefs.haptic_on_profile_switch = prefs_update.haptic_on_profile_switch;
        prefs
            .save(&state.preferences_path)
            .map_err(|e| e.to_string())?;
    }

    // Keep the atomic mirror in sync so the close handler can read it without block_on.
    state.close_to_tray.store(
        prefs_update.close_to_tray,
        std::sync::atomic::Ordering::Relaxed,
    );

    if start_at_login_changed {
        crate::login_item::set_start_at_login(prefs_update.start_at_login)?;
    }

    Ok(())
}

// ── check_for_update (desktop only) ──────────────────────────────────────────

/// Info about an available update, returned by [`check_for_update`].
#[cfg(not(mobile))]
#[derive(Serialize)]
pub struct UpdateInfoDto {
    /// The new version string, e.g. `"1.2.0"`.
    pub version: String,
    /// Markdown release notes from the update manifest, if present.
    pub release_notes: Option<String>,
}

/// Query the update endpoint and return info about the available update, or
/// `null` if the app is already on the latest version.
///
/// Does not download anything — call [`download_and_install_update`] for that.
#[cfg(not(mobile))]
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<Option<UpdateInfoDto>, String> {
    use tauri_plugin_updater::UpdaterExt as _;
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(update.map(|u| UpdateInfoDto {
        version: u.version.clone(),
        release_notes: u.body.clone(),
    }))
}

// ── download_and_install_update ───────────────────────────────────────────────

/// Download and install the latest available update, then restart the app.
///
/// Emits [`crate::events::UPDATE_DOWNLOAD_PROGRESS`] events periodically during
/// the download so the frontend can render a progress bar. Returns an error if
/// there is no update available or the download fails. Never returns on success
/// because `app.restart()` terminates the process.
#[cfg(not(mobile))]
#[tauri::command]
pub async fn download_and_install_update(app: tauri::AppHandle) -> Result<(), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use tauri::Emitter as _;
    use tauri_plugin_updater::UpdaterExt as _;

    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No update available".to_string())?;

    let downloaded = Arc::new(AtomicU64::new(0));
    let downloaded_clone = Arc::clone(&downloaded);
    let app_clone = app.clone();

    update
        .download_and_install(
            move |chunk_len, total| {
                let so_far =
                    downloaded_clone.fetch_add(chunk_len as u64, Ordering::Relaxed) + chunk_len as u64;
                let _ = app_clone.emit(
                    crate::events::UPDATE_DOWNLOAD_PROGRESS,
                    crate::events::UpdateProgressPayload { downloaded: so_far, total },
                );
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    app.restart();
}

// ── get_platform ─────────────────────────────────────────────────────────────

/// Return the current platform identifier.
///
/// Used by the Svelte frontend to conditionally show platform-specific UI
/// (e.g. hide tray settings on Android, show accessibility setup on Android).
#[tauri::command]
pub fn get_platform() -> String {
    #[cfg(target_os = "android")]
    return "android".into();
    #[cfg(target_os = "ios")]
    return "ios".into();
    #[cfg(target_os = "linux")]
    return "linux".into();
    #[cfg(target_os = "windows")]
    return "windows".into();
    #[cfg(target_os = "macos")]
    return "macos".into();
    #[allow(unreachable_code)]
    "unknown".into()
}

// ── read_file_text ────────────────────────────────────────────────────────────

/// Read a file at an absolute path and return its contents as a UTF-8 string.
///
/// Used by the frontend to read files dropped onto the window via drag-and-drop:
/// on Linux/WebKitGTK the WebView receives a `file://` URI rather than a `File`
/// object, so the frontend resolves the path and delegates the read to this command.
#[tauri::command]
pub fn read_file_text(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("could not read '{path}': {e}"))
}

// ── Unit tests ────────────────────────────────────────────────────────────────

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
    fn rename_device_empty_name_returns_error() {
        assert!(validate_device_name("").is_err());
        assert!(validate_device_name("   ").is_err());
    }

    #[test]
    fn rename_device_too_long_returns_error() {
        let long = "a".repeat(21);
        assert!(validate_device_name(&long).is_err());
    }

    #[test]
    fn rename_device_max_length_is_accepted() {
        let exactly_20 = "a".repeat(20);
        assert!(validate_device_name(&exactly_20).is_ok());
    }

    #[test]
    fn rename_device_non_ascii_returns_error() {
        assert!(validate_device_name("Tàp").is_err());
        assert!(validate_device_name("Tap🎹").is_err());
    }

    #[test]
    fn rename_device_control_char_returns_error() {
        assert!(validate_device_name("Tap\x00Dev").is_err());
        assert!(validate_device_name("Tap\nDev").is_err());
    }

    #[test]
    fn rename_device_whitespace_only_returns_error() {
        assert!(validate_device_name("\t\n ").is_err());
    }

    #[test]
    fn rename_device_valid_name_trims_whitespace() {
        let result = validate_device_name("  MyTap  ").expect("should be ok");
        assert_eq!(result, "MyTap");
    }

    #[test]
    fn rename_device_valid_ascii_name_accepted() {
        assert!(validate_device_name("TapXR_A036320").is_ok());
        assert!(validate_device_name("MyTap").is_ok());
        assert!(validate_device_name("Tap-1 (left)").is_ok());
    }

    #[test]
    fn tap_device_info_dto_from_device_info() {
        use std::str::FromStr as _;
        let info = tap_ble::TapDeviceInfo {
            name: Some("Tap Strap 2".into()),
            address: BDAddr::from_str("AA:BB:CC:DD:EE:FF").unwrap(),
            rssi: Some(-60),
            seen_in_scan: true,
            is_connected_to_os: false,
        };
        let dto = TapDeviceInfoDto::from(&info);
        assert_eq!(dto.address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(dto.name.as_deref(), Some("Tap Strap 2"));
        assert_eq!(dto.rssi, Some(-60));
        assert!(dto.seen_in_scan);
        assert!(!dto.is_connected_to_os);
    }
}
