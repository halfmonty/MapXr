use std::path::{Path, PathBuf};
use std::sync::Arc;

use mapping_core::{engine::ComboEngine, LayerRegistry};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::platform;

#[cfg(not(mobile))]
use std::sync::atomic::AtomicBool;
#[cfg(not(mobile))]
use std::time::Duration;
#[cfg(not(mobile))]
use tap_ble::{BleManager, BleStatusEvent, DeviceRegistry};
#[cfg(not(mobile))]
use tokio::sync::broadcast;
#[cfg(not(mobile))]
use crate::context_rules::ContextRules;

// ── AppState ─────────────────────────────────────────────────────────────────

/// Tauri managed state — holds every backend component.
///
/// Register once with `tauri::Builder::default().manage(state)` and receive in
/// commands via `tauri::State<'_, AppState>`.
///
/// ## Lock ordering
///
/// When more than one lock must be held simultaneously, always acquire in this
/// order to prevent deadlock:
///
/// 1. `engine`
/// 2. `layer_registry`
/// 3. `ble_manager`
///
/// `device_registry` uses `std::sync::Mutex` (sync I/O only) and must never
/// be held while awaiting another async operation.
pub struct AppState {
    /// Combo resolution engine.
    // LOCKING: tokio::sync::Mutex — push_event is called from an async task.
    pub engine: Mutex<ComboEngine>,

    /// On-disk map of all available profile `.json` files.
    // LOCKING: tokio::sync::Mutex — reload() does blocking I/O in async context.
    pub layer_registry: Mutex<LayerRegistry>,

    /// BLE adapter + connected device lifecycle (desktop only; Android uses Kotlin BLE plugin).
    ///
    /// `None` when no Bluetooth adapter was found at startup; all BLE commands
    /// return a descriptive error in that case.
    // LOCKING: tokio::sync::Mutex — scan/connect are async and long-running.
    #[cfg(not(mobile))]
    pub ble_manager: Option<Mutex<BleManager>>,

    /// Role → BLE address persistence (desktop only).
    #[cfg(not(mobile))]
    pub device_registry: std::sync::Mutex<DeviceRegistry>,

    /// Absolute path to the `profiles/` directory.
    pub profiles_dir: PathBuf,

    /// Absolute path to `devices.json` (desktop only — Android BLE registry lives in Kotlin).
    #[cfg(not(mobile))]
    pub devices_json_path: PathBuf,

    /// Absolute path to `preferences.json` (sibling of `devices.json`).
    pub preferences_path: PathBuf,

    /// Rules for automatic profile activation on window focus change (desktop only).
    // LOCKING: acquire after engine, layer_registry, and ble_manager.
    #[cfg(not(mobile))]
    pub context_rules: Mutex<ContextRules>,

    /// Absolute path to `context-rules.json` (desktop only).
    #[cfg(not(mobile))]
    pub context_rules_path: PathBuf,

    /// In-memory preferences, mirrored to `preferences_path` on change.
    // LOCKING: tokio::sync::Mutex — updated from async command handlers.
    pub(crate) preferences: Mutex<Preferences>,

    /// Mirror of `preferences.close_to_tray` as an atomic so the window close
    /// handler can read it without an async `block_on` call (desktop only).
    #[cfg(not(mobile))]
    pub(crate) close_to_tray: Arc<AtomicBool>,
}

impl AppState {
    /// Returns an error string suitable for a Tauri command if BLE is unavailable.
    #[cfg(not(mobile))]
    pub fn require_ble(&self) -> Result<(), String> {
        if self.ble_manager.is_none() {
            Err("No Bluetooth adapter found on this system".into())
        } else {
            Ok(())
        }
    }
}

// ── build_app_state ───────────────────────────────────────────────────────────

/// Initialise [`AppState`] from the Tauri app handle (desktop).
///
/// Returns `(AppState, event_rx, status_rx)` so the event pump and BLE status
/// listener tasks can be set up before the state is moved into Tauri's managed
/// state.  Either receiver may immediately yield `RecvError::Closed` if no BLE
/// adapter is available; both pump tasks handle this gracefully by exiting.
#[cfg(not(mobile))]
pub async fn build_app_state(
    app: &tauri::AppHandle,
) -> Result<
    (
        AppState,
        broadcast::Receiver<mapping_core::engine::RawTapEvent>,
        broadcast::Receiver<BleStatusEvent>,
    ),
    anyhow::Error,
> {
    let profiles_dir = platform::profile_dir(app).map_err(anyhow::Error::msg)?;
    let config_dir = profiles_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("profiles_dir has no parent"))?;
    let devices_json_path = config_dir.join("devices.json");
    let preferences_path = config_dir.join("preferences.json");
    let context_rules_path = config_dir.join("context-rules.json");

    // Load device registry (empty if file absent).
    let device_registry = DeviceRegistry::load(&devices_json_path)
        .map_err(|e| anyhow::anyhow!("failed to load device registry: {e}"))?;

    // Load context rules (empty if file absent or malformed).
    let context_rules = ContextRules::load(&context_rules_path);

    // Load user preferences (empty/default if file absent).
    let preferences = Preferences::load(&preferences_path);

    // Seed default profiles on first launch (no-op if any .json files exist).
    seed_profiles_dir(&profiles_dir);

    // Load layer registry (scan profiles_dir, ignore empty-dir errors).
    let mut layer_registry = LayerRegistry::new(&profiles_dir);
    let _ = layer_registry.reload();

    // Select the startup profile.
    //
    // If the user explicitly deactivated all profiles before closing, honour
    // that and start with the built-in empty profile (no mappings).
    //
    // Otherwise prefer the last explicitly-activated profile, fall back to
    // alphabetically-first (covers first launch), then to the built-in default.
    let default_profile = if preferences.profile_active {
        preferences
            .last_active_profile_id
            .as_deref()
            .and_then(|id| layer_registry.get(id))
            .cloned()
            .or_else(|| layer_registry.profiles().min_by_key(|p| &p.name).cloned())
            .unwrap_or_else(builtin_default_profile)
    } else {
        builtin_default_profile()
    };

    let engine = ComboEngine::new(default_profile.clone());

    // Attempt to acquire a BLE adapter.
    let (ble_manager_opt, event_rx, status_rx) = match BleManager::new().await {
        Ok(manager) => {
            BleManager::check_roles(&default_profile, &device_registry);
            let event_rx = manager.subscribe();
            let status_rx = manager.subscribe_status();
            (Some(Mutex::new(manager)), event_rx, status_rx)
        }
        Err(e) => {
            log::warn!("BLE adapter not available at startup: {e}");
            // Create a pair of already-closed channels so the pump tasks exit
            // immediately instead of blocking.
            let (event_tx, event_rx) = broadcast::channel::<mapping_core::engine::RawTapEvent>(1);
            let (status_tx, status_rx) = broadcast::channel::<BleStatusEvent>(1);
            drop(event_tx);
            drop(status_tx);
            (None, event_rx, status_rx)
        }
    };

    let close_to_tray = Arc::new(AtomicBool::new(preferences.close_to_tray));
    let state = AppState {
        engine: Mutex::new(engine),
        layer_registry: Mutex::new(layer_registry),
        ble_manager: ble_manager_opt,
        device_registry: std::sync::Mutex::new(device_registry),
        profiles_dir,
        devices_json_path,
        preferences_path,
        context_rules: Mutex::new(context_rules),
        context_rules_path,
        preferences: Mutex::new(preferences),
        close_to_tray,
    };

    Ok((state, event_rx, status_rx))
}

/// Initialise [`AppState`] from the Tauri app handle (Android).
///
/// The Android build has no BLE manager, device registry, or context rules —
/// those are handled by Kotlin plugins. Returns just the `AppState`.
#[cfg(mobile)]
pub async fn build_app_state(app: &tauri::AppHandle) -> Result<AppState, anyhow::Error> {
    let profiles_dir = platform::profile_dir(app).map_err(anyhow::Error::msg)?;
    let preferences_path = profiles_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("profiles_dir has no parent"))?
        .join("preferences.json");

    let preferences = Preferences::load(&preferences_path);

    seed_profiles_dir(&profiles_dir);

    let mut layer_registry = LayerRegistry::new(&profiles_dir);
    let _ = layer_registry.reload();

    let default_profile = if preferences.profile_active {
        preferences
            .last_active_profile_id
            .as_deref()
            .and_then(|id| layer_registry.get(id))
            .cloned()
            .or_else(|| layer_registry.profiles().min_by_key(|p| &p.name).cloned())
            .unwrap_or_else(builtin_default_profile)
    } else {
        builtin_default_profile()
    };

    let engine = ComboEngine::new(default_profile);

    Ok(AppState {
        engine: Mutex::new(engine),
        layer_registry: Mutex::new(layer_registry),
        profiles_dir,
        preferences_path,
        preferences: Mutex::new(preferences),
    })
}

// ── Auto-reconnect (desktop only) ────────────────────────────────────────────

#[cfg(not(mobile))]
/// Attempt to reconnect all devices saved in the device registry.
///
/// Runs as a background task immediately after app state is registered.
/// Always scans first to populate the adapter's peripheral cache, then
/// connects all registered devices in sequence.
///
/// Scanning before any connection attempt avoids a Linux/BlueZ issue where
/// running a scan while a device is already mid-connection can disrupt the
/// active connection — previously causing only the last-connected device to
/// survive when two devices were registered.
///
/// Failures are logged and skipped — they never block startup.
#[cfg(not(mobile))]
pub(crate) async fn auto_reconnect(app: tauri::AppHandle, state: Arc<AppState>) {
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
    const SCAN_DURATION_MS: u64 = 5000;

    let Some(ble) = &state.ble_manager else {
        return;
    };

    // Snapshot the registry while holding the sync lock briefly.
    let saved: Vec<_> = {
        let reg = match state.device_registry.lock() {
            Ok(r) => r,
            Err(_) => return,
        };
        reg.iter().map(|(id, addr)| (id.clone(), *addr)).collect()
    };

    if saved.is_empty() {
        return;
    }

    log::info!(
        "auto_reconnect: {} saved device(s), scanning first",
        saved.len()
    );

    // Scan before connecting. This populates the adapter cache and avoids
    // triggering a scan mid-connection which can drop already-connected devices
    // on some Linux Bluetooth adapters.
    {
        let mut manager = ble.lock().await;
        if let Err(e) = manager.scan(SCAN_DURATION_MS).await {
            // Non-fatal: bonded devices may still be reachable from the BlueZ cache.
            log::warn!("auto_reconnect: scan failed: {e}");
        }
    }

    for (device_id, address) in &saved {
        let result = {
            let mut manager = ble.lock().await;
            tokio::time::timeout(
                CONNECT_TIMEOUT,
                manager.connect(device_id.clone(), *address),
            )
            .await
        };
        match result {
            Ok(Ok(())) => {
                log::info!("auto_reconnect: connected {device_id} ({address})");
                crate::pump::emit_layer_changed(&app, &state).await;
            }
            Ok(Err(e)) => {
                log::warn!("auto_reconnect: failed {device_id} ({address}): {e}");
            }
            Err(_) => {
                log::warn!("auto_reconnect: timed out {device_id} ({address})");
            }
        }
    }
}

// ── Starter profile seeding ───────────────────────────────────────────────────

/// Embedded starter profiles shipped with the application.
///
/// Each entry is `(filename, json_bytes)`. The JSON is compiled into the binary
/// so no additional files need to be bundled at runtime.
const STARTER_PROFILES: &[(&str, &str)] = &[(
    "starter-right.json",
    include_str!("../profiles/starter-right.json"),
)];

/// Write starter profiles into `profiles_dir` if it contains no `.json` files.
///
/// Called once at startup. If the user has already created profiles (or deleted
/// the starters) this function is a no-op.
fn seed_profiles_dir(profiles_dir: &std::path::Path) {
    let has_json = std::fs::read_dir(profiles_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        })
        .unwrap_or(false);

    if has_json {
        return;
    }

    for (filename, contents) in STARTER_PROFILES {
        let path = profiles_dir.join(filename);
        match std::fs::write(&path, contents) {
            Ok(()) => log::info!("seeded starter profile: {filename}"),
            Err(e) => log::warn!("failed to seed starter profile {filename}: {e}"),
        }
    }
}

// ── Preferences ───────────────────────────────────────────────────────────────

/// On-disk representation of `preferences.json`.
#[derive(Serialize, Deserialize)]
struct StoredPreferences {
    version: u32,
    /// `false` only when the user has explicitly called `deactivate_profile`.
    /// Absent in older files (before this field was added); treated as `true`
    /// so existing installations keep their current behaviour.
    #[serde(default = "default_true")]
    profile_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_active_profile_id: Option<String>,
    /// Hide window on close instead of quitting.  Default `true`.
    #[serde(default = "default_true")]
    close_to_tray: bool,
    /// Launch directly to tray without showing the main window.  Default `false`.
    #[serde(default)]
    start_minimised: bool,
    /// Register the app to start at OS login.  Default `false`.
    #[serde(default)]
    start_at_login: bool,
    /// Whether the first-hide "still running" notification has been shown.
    #[serde(default)]
    shown_tray_hint: bool,
    /// Notify when a Tap device connects.  Default `true`.
    #[serde(default = "default_true")]
    notify_device_connected: bool,
    /// Notify when a Tap device disconnects.  Default `true`.
    #[serde(default = "default_true")]
    notify_device_disconnected: bool,
    /// Notify when the active layer switches.  Default `false`.
    #[serde(default)]
    notify_layer_switch: bool,
    /// Notify when the active profile switches.  Default `true`.
    #[serde(default = "default_true")]
    notify_profile_switch: bool,
    /// Master haptic toggle — gates all vibration dispatch.  Default `true`.
    #[serde(default = "default_true")]
    haptics_enabled: bool,
    /// Vibrate on every resolved tap event.  Default `false`.
    #[serde(default)]
    haptic_on_tap: bool,
    /// Vibrate on layer push/pop/switch.  Default `true`.
    #[serde(default = "default_true")]
    haptic_on_layer_switch: bool,
    /// Vibrate on profile activate.  Default `true`.
    #[serde(default = "default_true")]
    haptic_on_profile_switch: bool,
}

fn default_true() -> bool {
    true
}

/// User preferences persisted across sessions.
///
/// Stored as `preferences.json` in the app config directory alongside
/// `devices.json`.  Written on profile activation/deactivation and when tray
/// settings change.
pub(crate) struct Preferences {
    /// `false` only when the user has explicitly deactivated all profiles.
    ///
    /// Defaults to `true` so that first launch (no preferences file) still
    /// picks a sensible startup profile.
    pub profile_active: bool,
    /// The `layer_id` of the last profile the user explicitly activated.
    pub last_active_profile_id: Option<String>,
    /// Hide window on close instead of quitting (default `true`).
    pub close_to_tray: bool,
    /// Launch directly to tray without showing the main window (default `false`).
    pub start_minimised: bool,
    /// Register the app to start at OS login (default `false`).
    pub start_at_login: bool,
    /// Whether the first-hide "still running" notification has been shown.
    pub shown_tray_hint: bool,
    /// Notify when a Tap device connects (default `true`).
    pub notify_device_connected: bool,
    /// Notify when a Tap device disconnects (default `true`).
    pub notify_device_disconnected: bool,
    /// Notify when the active layer switches (default `false`).
    pub notify_layer_switch: bool,
    /// Notify when the active profile switches (default `true`).
    pub notify_profile_switch: bool,
    /// Master haptic toggle — gates all vibration (default `true`).
    pub haptics_enabled: bool,
    /// Vibrate on every resolved tap event (default `false`).
    pub haptic_on_tap: bool,
    /// Vibrate on layer push/pop/switch (default `true`).
    pub haptic_on_layer_switch: bool,
    /// Vibrate on profile activate (default `true`).
    pub haptic_on_profile_switch: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            profile_active: true,
            last_active_profile_id: None,
            close_to_tray: true,
            start_minimised: false,
            start_at_login: false,
            shown_tray_hint: false,
            notify_device_connected: true,
            notify_device_disconnected: true,
            notify_layer_switch: false,
            notify_profile_switch: true,
            haptics_enabled: true,
            haptic_on_tap: false,
            haptic_on_layer_switch: true,
            haptic_on_profile_switch: true,
        }
    }
}

impl Preferences {
    /// Load from `path`.  Returns a default `Preferences` if the file does not
    /// exist or cannot be parsed; errors are logged but not propagated so a
    /// corrupt preferences file never prevents the app from starting.
    pub fn load(path: &Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                log::warn!("failed to read preferences: {e}");
                return Self::default();
            }
        };
        match serde_json::from_str::<StoredPreferences>(&text) {
            Ok(stored) => Self {
                profile_active: stored.profile_active,
                last_active_profile_id: stored.last_active_profile_id,
                close_to_tray: stored.close_to_tray,
                start_minimised: stored.start_minimised,
                start_at_login: stored.start_at_login,
                shown_tray_hint: stored.shown_tray_hint,
                notify_device_connected: stored.notify_device_connected,
                notify_device_disconnected: stored.notify_device_disconnected,
                notify_layer_switch: stored.notify_layer_switch,
                notify_profile_switch: stored.notify_profile_switch,
                haptics_enabled: stored.haptics_enabled,
                haptic_on_tap: stored.haptic_on_tap,
                haptic_on_layer_switch: stored.haptic_on_layer_switch,
                haptic_on_profile_switch: stored.haptic_on_profile_switch,
            },
            Err(e) => {
                log::warn!("failed to parse preferences: {e}");
                Self::default()
            }
        }
    }

    /// Persist to `path` using a write-then-rename strategy for atomicity.
    pub fn save(&self, path: &Path) -> Result<(), anyhow::Error> {
        let stored = StoredPreferences {
            version: 2,
            profile_active: self.profile_active,
            last_active_profile_id: self.last_active_profile_id.clone(),
            close_to_tray: self.close_to_tray,
            start_minimised: self.start_minimised,
            start_at_login: self.start_at_login,
            shown_tray_hint: self.shown_tray_hint,
            notify_device_connected: self.notify_device_connected,
            notify_device_disconnected: self.notify_device_disconnected,
            notify_layer_switch: self.notify_layer_switch,
            notify_profile_switch: self.notify_profile_switch,
            haptics_enabled: self.haptics_enabled,
            haptic_on_tap: self.haptic_on_tap,
            haptic_on_layer_switch: self.haptic_on_layer_switch,
            haptic_on_profile_switch: self.haptic_on_profile_switch,
        };
        let json = serde_json::to_string_pretty(&stored)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// A minimal valid profile used when no profile files are present at startup,
/// and as the deactivated state when no user profile should be active.
pub(crate) fn builtin_default_profile() -> mapping_core::types::Profile {
    use mapping_core::types::{Hand, Profile, ProfileKind, ProfileSettings};
    Profile {
        version: 1,
        kind: ProfileKind::Single,
        name: "Default".into(),
        description: None,
        layer_id: "default".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Notification-gating defaults ──────────────────────────────────────────

    #[test]
    fn preferences_default_notify_device_connected_is_true() {
        assert!(Preferences::default().notify_device_connected);
    }

    #[test]
    fn preferences_default_notify_device_disconnected_is_true() {
        assert!(Preferences::default().notify_device_disconnected);
    }

    #[test]
    fn preferences_default_notify_layer_switch_is_false() {
        assert!(!Preferences::default().notify_layer_switch);
    }

    #[test]
    fn preferences_default_notify_profile_switch_is_true() {
        assert!(Preferences::default().notify_profile_switch);
    }

    #[test]
    fn preferences_load_missing_notify_fields_falls_back_to_defaults() {
        // A stored preferences JSON without notification fields must produce the
        // correct defaults when loaded (backwards compatibility).
        let json = r#"{"version":2,"close_to_tray":true,"start_minimised":false,"start_at_login":false,"shown_tray_hint":false}"#;
        let stored: StoredPreferences = serde_json::from_str(json).expect("should parse");
        assert!(stored.notify_device_connected); // default_true
        assert!(stored.notify_device_disconnected); // default_true
        assert!(!stored.notify_layer_switch); // default false
        assert!(stored.notify_profile_switch); // default_true
    }

    #[test]
    fn preferences_load_explicit_notify_fields_are_respected() {
        let json = r#"{
            "version": 2,
            "notify_device_connected": false,
            "notify_device_disconnected": false,
            "notify_layer_switch": true,
            "notify_profile_switch": false
        }"#;
        let stored: StoredPreferences = serde_json::from_str(json).expect("should parse");
        assert!(!stored.notify_device_connected);
        assert!(!stored.notify_device_disconnected);
        assert!(stored.notify_layer_switch);
        assert!(!stored.notify_profile_switch);
    }

    // ── Haptic defaults ───────────────────────────────────────────────────────

    #[test]
    fn preferences_default_haptics_enabled_is_true() {
        assert!(Preferences::default().haptics_enabled);
    }

    #[test]
    fn preferences_default_haptic_on_tap_is_false() {
        assert!(!Preferences::default().haptic_on_tap);
    }

    #[test]
    fn preferences_default_haptic_on_layer_switch_is_true() {
        assert!(Preferences::default().haptic_on_layer_switch);
    }

    #[test]
    fn preferences_default_haptic_on_profile_switch_is_true() {
        assert!(Preferences::default().haptic_on_profile_switch);
    }

    #[test]
    fn preferences_load_missing_haptic_fields_falls_back_to_defaults() {
        // Older preferences.json without haptic fields should get correct defaults.
        let json = r#"{"version":2,"close_to_tray":true,"start_minimised":false,"start_at_login":false,"shown_tray_hint":false}"#;
        let stored: StoredPreferences = serde_json::from_str(json).expect("should parse");
        assert!(stored.haptics_enabled);         // default_true
        assert!(!stored.haptic_on_tap);          // default false
        assert!(stored.haptic_on_layer_switch);  // default_true
        assert!(stored.haptic_on_profile_switch); // default_true
    }

    #[test]
    fn preferences_load_explicit_haptic_fields_are_respected() {
        let json = r#"{
            "version": 2,
            "haptics_enabled": false,
            "haptic_on_tap": true,
            "haptic_on_layer_switch": false,
            "haptic_on_profile_switch": false
        }"#;
        let stored: StoredPreferences = serde_json::from_str(json).expect("should parse");
        assert!(!stored.haptics_enabled);
        assert!(stored.haptic_on_tap);
        assert!(!stored.haptic_on_layer_switch);
        assert!(!stored.haptic_on_profile_switch);
    }

    // ── Starter profiles ──────────────────────────────────────────────────────

    #[test]
    fn starter_profiles_all_parse_and_validate() {
        for (filename, contents) in STARTER_PROFILES {
            let profile: mapping_core::types::Profile = serde_json::from_str(contents)
                .unwrap_or_else(|e| panic!("{filename}: failed to deserialise: {e}"));
            profile
                .validate()
                .unwrap_or_else(|e| panic!("{filename}: failed validation: {e}"));
        }
    }
}
