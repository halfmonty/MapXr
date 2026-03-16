use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mapping_core::{engine::ComboEngine, LayerRegistry};
use tap_ble::{BleManager, BleStatusEvent, DeviceRegistry};
use tokio::sync::{broadcast, Mutex};

use crate::platform;

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

    /// BLE adapter + connected device lifecycle.
    ///
    /// `None` when no Bluetooth adapter was found at startup; all BLE commands
    /// return a descriptive error in that case.
    // LOCKING: tokio::sync::Mutex — scan/connect are async and long-running.
    pub ble_manager: Option<Mutex<BleManager>>,

    /// Role → BLE address persistence (sync I/O, brief lock).
    pub device_registry: std::sync::Mutex<DeviceRegistry>,

    /// Absolute path to the `profiles/` directory.
    pub profiles_dir: PathBuf,

    /// Absolute path to `devices.json` (parent of `profiles_dir`).
    pub devices_json_path: PathBuf,
}

impl AppState {
    /// Returns an error string suitable for a Tauri command if BLE is unavailable.
    pub fn require_ble(&self) -> Result<(), String> {
        if self.ble_manager.is_none() {
            Err("No Bluetooth adapter found on this system".into())
        } else {
            Ok(())
        }
    }
}

// ── build_app_state ───────────────────────────────────────────────────────────

/// Initialise [`AppState`] from the Tauri app handle.
///
/// Returns `(AppState, event_rx, status_rx)` so the event pump and BLE status
/// listener tasks can be set up before the state is moved into Tauri's managed
/// state.  Either receiver may immediately yield `RecvError::Closed` if no BLE
/// adapter is available; both pump tasks handle this gracefully by exiting.
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
    let devices_json_path = profiles_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("profiles_dir has no parent"))?
        .join("devices.json");

    // Load device registry (empty if file absent).
    let device_registry = DeviceRegistry::load(&devices_json_path)
        .map_err(|e| anyhow::anyhow!("failed to load device registry: {e}"))?;

    // Load layer registry (scan profiles_dir, ignore empty-dir errors).
    let mut layer_registry = LayerRegistry::new(&profiles_dir);
    let _ = layer_registry.reload();

    // Select default profile: first by name, or a minimal built-in.
    let default_profile = layer_registry
        .profiles()
        .min_by_key(|p| &p.name)
        .cloned()
        .unwrap_or_else(builtin_default_profile);

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

    let state = AppState {
        engine: Mutex::new(engine),
        layer_registry: Mutex::new(layer_registry),
        ble_manager: ble_manager_opt,
        device_registry: std::sync::Mutex::new(device_registry),
        profiles_dir,
        devices_json_path,
    };

    Ok((state, event_rx, status_rx))
}

// ── Auto-reconnect ────────────────────────────────────────────────────────────

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
        reg.iter()
            .map(|(id, addr)| (id.clone(), *addr))
            .collect()
    };

    if saved.is_empty() {
        return;
    }

    log::info!("auto_reconnect: {} saved device(s), scanning first", saved.len());

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
