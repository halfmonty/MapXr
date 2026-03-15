# tauri-commands — Tauri command layer specification (Epic 4)

## Table of contents

1. [Overview](#overview)
2. [Dependencies](#dependencies)
3. [Module structure](#module-structure)
4. [AppState design](#appstate-design)
5. [Initialisation](#initialisation)
6. [Event pump task](#event-pump-task)
7. [Commands (4a)](#commands-4a)
8. [Events (4b)](#events-4b)
9. [State management (4c)](#state-management-4c)
10. [Shutdown and cleanup](#shutdown-and-cleanup)
11. [BLE status notifications — tap-ble changes](#ble-status-notifications--tap-ble-changes)
12. [New types (serde-serialisable DTOs)](#new-types-serde-serialisable-dtos)
13. [Testing strategy](#testing-strategy)

---

## Overview

Epic 4 replaces the scaffold `greet` command with a full Tauri command layer that bridges the
Rust backend to the Svelte frontend. The layer owns:

- An `AppState` managed struct holding the engine, BLE manager, registries, and runtime paths.
- A background **event pump task** that receives `RawTapEvent`s from the BLE layer, drives
  `ComboEngine`, performs keyboard/mouse simulation (via `enigo`), and emits Tauri events.
- Thirteen `#[tauri::command]` handlers covering device scanning, profile management, engine
  control, and state introspection.
- Six Tauri event types emitted from the event pump and command handlers.
- Graceful shutdown: exit controller mode on all connected devices before the OS window closes.

`src-tauri` is a Tauri 2.x binary crate using edition 2021. All Rust here uses `anyhow` for
error handling (acceptable in binary crates per `CLAUDE.md`).

---

## Dependencies

Add to `src-tauri/Cargo.toml`:

| Crate | Purpose |
| ----- | ------- |
| `tap-ble = { path = "../crates/tap-ble" }` | BLE layer |
| `tokio = { version = "1", features = ["sync", "rt", "macros", "time"] }` | Async runtime; Tauri 2.x already depends on tokio but explicit dep is needed for `spawn` and `Mutex` |
| `env_logger = "0.11"` | Wire up the `log` facade (used by `tap-ble`) at process startup |
| `enigo = "0.2"` | Keyboard and mouse simulation |
| `anyhow = "1"` | Error handling in binary crate |

`serde` and `serde_json` are already present. `mapping-core` is already present.

> **Dependency approval required:** `env_logger`, `enigo`, `anyhow`, and the explicit `tokio`
> dep must be confirmed by the user before adding them. They are proposed here because they are
> implied by the spec's requirements. `env_logger` and `anyhow` are standard; `enigo` was
> pre-approved in `CLAUDE.md`.

---

## Module structure

```
src-tauri/src/
├── main.rs           — entry point (unchanged except env_logger init)
├── lib.rs            — run(); registers state, commands, event handlers, spawns event pump
├── platform.rs       — profile_dir() helper (already exists)
├── state.rs          — AppState struct and initialisation helper
├── commands.rs       — all #[tauri::command] functions
└── events.rs         — event name constants and payload types
```

---

## AppState design

```rust
pub struct AppState {
    /// BLE adapter + connected device registry.
    /// `tokio::sync::Mutex` because BLE ops (.scan, .connect) are async.
    pub ble_manager: tokio::sync::Mutex<tap_ble::BleManager>,

    /// Combo resolution engine.
    /// `tokio::sync::Mutex` because `push_event` is called from an async task.
    pub engine: tokio::sync::Mutex<mapping_core::engine::ComboEngine>,

    /// On-disk map of all available profile files.
    pub layer_registry: tokio::sync::Mutex<mapping_core::LayerRegistry>,

    /// Role → BLE address persistence.
    /// `std::sync::Mutex` because all ops are synchronous I/O.
    pub device_registry: std::sync::Mutex<tap_ble::DeviceRegistry>,

    /// Absolute path to the profiles directory (e.g. `~/.config/mapxr/profiles/`).
    pub profiles_dir: std::path::PathBuf,

    /// Absolute path to `devices.json` (sibling of `profiles_dir`).
    pub devices_json_path: std::path::PathBuf,

    /// Sender for BLE status events (connect / disconnect notifications).
    /// See §BLE status notifications.
    pub ble_status_rx_factory: tap_ble::BleStatusSender,
}
```

`AppState` is registered with Tauri's managed state system. Commands receive it via
`tauri::State<'_, AppState>`.

### Lock ordering

To prevent deadlock, always acquire locks in this order when multiple are needed:

1. `engine`
2. `layer_registry`
3. `device_registry`
4. `ble_manager`

In practice, no command or task needs more than one lock at a time. Document any exception with
a `// LOCKING:` comment.

---

## Initialisation

`state.rs` exposes:

```rust
pub async fn build_app_state(app: &tauri::AppHandle) -> Result<AppState, anyhow::Error>
```

Called once in `lib.rs::run()` before the Tauri builder `.manage()` call.

Steps:

1. Resolve `profiles_dir` via `platform::profile_dir(app)` (see override rule below).
2. Resolve `devices_json_path` as `profiles_dir.parent().unwrap().join("devices.json")`.
   (i.e., `~/.config/mapxr/devices.json` when using the OS path, or `./devices.json` when
   the local override is active).
3. Load `DeviceRegistry` via `DeviceRegistry::load(&devices_json_path)` (empty if missing).
4. Load `LayerRegistry` by scanning `profiles_dir`.
5. Determine the **default profile**: the first profile in `LayerRegistry` sorted by name, or
   a built-in empty single-hand profile if the registry is empty.
6. Construct `ComboEngine::new(default_profile)`.
7. Construct `BleManager::new()` (returns `BleError::AdapterNotFound` on headless CI — handle
   gracefully: log a warning and use a stub that fails all BLE commands with a descriptive error).
8. Call `BleManager::check_roles(&default_profile, &device_registry)` to emit any startup
   warnings.

### Local profiles/ override

`platform::profile_dir(app)` checks for a `profiles/` directory **next to the running
executable** (i.e. `std::env::current_exe()?.parent()?.join("profiles")`). If that directory
exists it is used as-is and the OS config path is ignored entirely. This lets a developer run
`cargo tauri dev` from the workspace root and have the engine pick up the `profiles/` directory
that already lives there, without touching `~/.config/mapxr/`.

```
Lookup order:
  1. <exe_dir>/profiles/    — exists? use it (dev override)
  2. <os_config_dir>/mapxr/profiles/   — always exists (created on demand)
```

The override is purely path-based; no environment variable or flag is required. The directory
is **not** created if it does not exist (it must already be present, so a stale empty directory
does not accidentally shadow the OS path with no profiles).

Update `platform::profile_dir` to implement this logic.

### BLE adapter failure at startup

If `BleManager::new()` returns `BleError::AdapterNotFound`, set a flag on `AppState`
(`ble_available: bool`) so that all BLE commands return a user-friendly error:
`"No Bluetooth adapter found on this system"` rather than panicking.

---

## Event pump task

A single long-running tokio task is spawned in `lib.rs::run()` after state is registered.

```rust
tokio::spawn(event_pump(app_handle.clone(), event_rx, state.clone()));
```

Where `event_rx` is a `broadcast::Receiver<RawTapEvent>` obtained from
`ble_manager.subscribe()` **before** `AppState` is moved into managed state.

### Event pump loop

```
loop {
    match event_rx.recv().await {
        Ok(raw_event) => {
            // 1. Lock engine briefly, push event, collect outputs
            let outputs = {
                let mut engine = state.engine.lock().await;
                engine.push_event(raw_event.clone(), Instant::now())
            };

            // 2. Emit tap-event for the live visualiser (always)
            app.emit("tap-event", TapEventPayload::from(&raw_event)).ok();

            // 3. Process each EngineOutput
            for output in outputs {
                // Emit debug-event if present
                if let Some(debug) = output.debug {
                    app.emit("debug-event", &debug).ok();
                }
                // Execute actions
                for action in output.actions {
                    execute_action(&action, &app, &mut enigo_handle);
                }
            }
        }
        Err(broadcast::error::RecvError::Lagged(n)) => {
            log::warn!("event pump lagged, dropped {n} events");
        }
        Err(broadcast::error::RecvError::Closed) => break,
    }
}
```

### Timeout polling

`ComboEngine::check_timeout` must be called periodically to flush pending sequence windows.
The event pump runs a secondary `tokio::time::interval` of **50 ms** (sufficient resolution
given the default `window_ms` of 200 ms) and calls `engine.check_timeout(Instant::now())`
on each tick, processing outputs the same way as live events. Use `tokio::select!` to poll
both the event receiver and the timeout tick concurrently.

### Enigo handle

`enigo::Enigo` is created once and owned by the event pump task (not shared state). It is not
`Clone`; keeping it local to the task avoids synchronisation overhead.

### `execute_action` helper

```rust
fn execute_action(action: &Action, app: &tauri::AppHandle, enigo: &mut enigo::Enigo)
```

Handles each `Action` variant:

| Action variant | Behaviour |
| -------------- | --------- |
| `Key { key, modifiers }` | Press modifiers, tap key, release modifiers via `enigo` |
| `KeyChord { keys, modifiers }` | Press all modifiers, press all keys in order, release all in reverse |
| `TypeString { text }` | `enigo.text(&text)` |
| `Macro { steps }` | For each step: `execute_action(step.action, …)`, then `tokio::time::sleep(step.delay_ms)` — spawn a detached task so the pump is not blocked |
| `PushLayer { layer_id, mode }` | Look up profile in `LayerRegistry`, lock engine, call `push_layer`; emit `layer-changed` event |
| `PopLayer` | Lock engine, call `pop_layer`; emit `layer-changed` event |
| `SwitchLayer { layer_id }` | Lock engine, call `switch_layer`; emit `layer-changed` event |
| `ToggleVariable { … }` | Lock engine, delegate to engine's variable logic; emit `layer-changed` |
| `SetVariable { … }` | Lock engine, delegate |
| `Block` | No-op (action is consumed; nothing emitted) |
| `Alias { name }` | Look up alias in current profile, recursively execute resolved action |

For layer/variable actions that need the `LayerRegistry`, access `app.state::<AppState>()`.

> **Note:** `Macro` steps contain `Action` values that may themselves be macro steps. The
> spec forbids nested macros (validated on profile load), so one level of spawn is sufficient.

---

## Commands (4a)

All commands are `async` and live in `commands.rs`. All fallible commands return
`Result<T, String>` — Tauri serialises the `Err` variant as a JSON string that the frontend
receives as an exception from `invoke`.

Errors are formatted as `anyhow::Error` internally and converted with `.to_string()` at the
command boundary.

### 4.1 `scan_devices`

```rust
#[tauri::command]
pub async fn scan_devices(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<TapDeviceInfoDto>, String>
```

- Acquires `ble_manager` lock, calls `scan(5000)`.
- Returns `Vec<TapDeviceInfoDto>` (see §New types).
- BLE unavailable → return `Err("No Bluetooth adapter found on this system")`.

### 4.2 `connect_device`

```rust
#[tauri::command]
pub async fn connect_device(
    state: tauri::State<'_, AppState>,
    address: String,
    role: String,
) -> Result<(), String>
```

- Parse `address` as `BDAddr` (return `Err` if invalid).
- Parse `role` as `DeviceId` (accept only `"solo"`, `"left"`, `"right"`; return `Err` otherwise).
- Acquire `ble_manager` lock, call `connect(device_id, address)`.
- On success: acquire `device_registry` lock, call `assign(device_id, address)`, call
  `save(&devices_json_path)`.
- The `device-connected` event is emitted by the BLE status listener (see §BLE status
  notifications), not here.

### 4.3 `disconnect_device`

```rust
#[tauri::command]
pub async fn disconnect_device(
    state: tauri::State<'_, AppState>,
    role: String,
) -> Result<(), String>
```

- Parse `role` → `DeviceId`.
- Acquire `ble_manager` lock, call `disconnect(&device_id)`.
- The `device-disconnected` event is emitted by the BLE status listener.

### 4.4 `list_profiles`

```rust
#[tauri::command]
pub async fn list_profiles(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ProfileSummary>, String>
```

- Acquires `layer_registry` lock, calls `reload()` to pick up any files added on disk.
- Returns a `Vec<ProfileSummary>` sorted by `name` ascending.

### 4.5 `load_profile`

```rust
#[tauri::command]
pub async fn load_profile(
    state: tauri::State<'_, AppState>,
    layer_id: String,
) -> Result<mapping_core::types::Profile, String>
```

- Acquires `layer_registry` lock, looks up `layer_id`.
- Returns `Err("Profile not found: {layer_id}")` if absent.
- The returned `Profile` must already implement `serde::Serialize` (it does, via `mapping-core`).

### 4.6 `save_profile`

```rust
#[tauri::command]
pub async fn save_profile(
    state: tauri::State<'_, AppState>,
    profile: mapping_core::types::Profile,
) -> Result<(), String>
```

- Determines target path: `profiles_dir / "{profile.layer_id}.json"`.
- Calls `profile.save(&path)`.
- Acquires `layer_registry` lock, calls `reload()` so the registry is up-to-date.

### 4.7 `delete_profile`

```rust
#[tauri::command]
pub async fn delete_profile(
    state: tauri::State<'_, AppState>,
    layer_id: String,
) -> Result<(), String>
```

- Constructs path `profiles_dir / "{layer_id}.json"`.
- Calls `std::fs::remove_file`. Returns `Err` if file not found or I/O error.
- Acquires `layer_registry` lock, calls `reload()`.

### 4.8 `activate_profile`

```rust
#[tauri::command]
pub async fn activate_profile(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    layer_id: String,
) -> Result<(), String>
```

- Acquires `layer_registry` lock, looks up `layer_id`. Returns `Err` if absent.
- Acquires `engine` lock, constructs a new `ComboEngine::new(profile.clone())` and replaces
  the existing engine (i.e., `*engine = ComboEngine::new(profile)`).
- Emits `layer-changed` event with the new single-entry layer stack.

> **Note on engine replacement:** `ComboEngine` does not have a `reset` method. The approach
> of replacing the engine with `*engine_guard = ComboEngine::new(profile)` is intentional —
> it clears all pending state. The `engine` field is `tokio::sync::Mutex<ComboEngine>`, so
> the lock guard can be dereferenced to replace the value.

### 4.9 `push_layer`

```rust
#[tauri::command]
pub async fn push_layer(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    layer_id: String,
    mode: mapping_core::types::PushLayerMode,
) -> Result<(), String>
```

- Acquires `layer_registry` lock, looks up `layer_id`.
- Acquires `engine` lock, calls `engine.push_layer(profile, mode)`, collects outputs.
- Processes outputs (execute actions, emit `layer-changed`).

### 4.10 `pop_layer`

```rust
#[tauri::command]
pub async fn pop_layer(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String>
```

- Acquires `engine` lock, calls `engine.pop_layer()`, collects outputs.
- Processes outputs (execute actions, emit `layer-changed`).
- Returns `Err("Layer stack is at base layer; nothing to pop")` if `pop_layer()` returns an
  empty output (meaning no layer was popped — check if the stack has depth > 1 before
  calling, or detect from the output).

### 4.11 `set_debug_mode`

```rust
#[tauri::command]
pub async fn set_debug_mode(
    state: tauri::State<'_, AppState>,
    enabled: bool,
)
```

- Acquires `engine` lock, sets `engine.debug_mode = enabled`.
- No return value (infallible).

### 4.12 `get_engine_state`

```rust
#[tauri::command]
pub async fn get_engine_state(
    state: tauri::State<'_, AppState>,
) -> Result<EngineStateSnapshot, String>
```

- Acquires `engine` lock, reads `engine.layer_stack.layer_ids()` and variable state.
- Acquires `ble_manager` lock, reads `connected_ids()`.
- Returns `EngineStateSnapshot` (see §New types).

---

## Events (4b)

Event constants live in `events.rs`. Payloads implement `serde::Serialize`.

### 4.13 `tap-event`

Emitted by the event pump on every `RawTapEvent`.

```rust
pub const TAP_EVENT: &str = "tap-event";

#[derive(Serialize)]
pub struct TapEventPayload {
    pub device_id: String,
    pub tap_code: u8,
    pub received_at_ms: u64,   // millis since Unix epoch, for JS Date construction
}
```

### 4.14 `action-fired`

Emitted by `execute_action` for every action dispatched to the platform layer
(keyboard simulation or Tauri event).

```rust
pub const ACTION_FIRED: &str = "action-fired";

#[derive(Serialize)]
pub struct ActionFiredPayload {
    pub action_kind: String,   // "key", "type_string", "push_layer", etc.
    pub label: Option<String>, // mapping label if available (from EngineOutput context)
}
```

> **Implementation note:** `EngineOutput` does not currently carry a mapping label.
> For task 4.14, emit the `action_kind` (derived from the `Action` enum variant name)
> and set `label: None` for now. The label field is reserved for a future improvement.

### 4.15 `layer-changed`

Emitted whenever the layer stack changes (push, pop, switch, activate).

```rust
pub const LAYER_CHANGED: &str = "layer-changed";

#[derive(Serialize)]
pub struct LayerChangedPayload {
    /// Layer IDs from bottom (base) to top.
    pub stack: Vec<String>,
    /// `layer_id` of the currently active (top) layer.
    pub active: String,
}
```

### 4.16 `device-connected` / `device-disconnected`

Emitted by the BLE status listener task (not the command handler — see §BLE status
notifications).

```rust
pub const DEVICE_CONNECTED: &str    = "device-connected";
pub const DEVICE_DISCONNECTED: &str = "device-disconnected";

#[derive(Serialize)]
pub struct DeviceStatusPayload {
    pub role: String,          // "solo", "left", "right"
    pub address: String,       // "AA:BB:CC:DD:EE:FF"
    pub name: Option<String>,  // device advertising name if known
}
```

### 4.17 `debug-event`

Emitted by the event pump when `output.debug` is `Some`.

```rust
pub const DEBUG_EVENT: &str = "debug-event";
```

Payload: `mapping_core::engine::DebugEvent` serialised directly. `Serialize` and
`Deserialize` have been added to `DebugEvent` in `mapping-core` as part of the spec
approval step.

### 4.18 `profile-error`

Emitted when a profile fails to load during `LayerRegistry::reload()`.

```rust
pub const PROFILE_ERROR: &str = "profile-error";

#[derive(Serialize)]
pub struct ProfileErrorPayload {
    pub file_name: String,
    pub message: String,
}
```

> **Implementation note:** `LayerRegistry::reload()` currently silently skips invalid files.
> For task 4.18, modify `LayerRegistry` to return a list of load errors alongside the success
> results, so the Tauri layer can emit `profile-error` for each failed file.

---

## State management (4c)

### 4.19 `AppState` struct

Defined in `state.rs` as described in §AppState design. Registered via:

```rust
tauri::Builder::default()
    .manage(app_state)
    ...
```

### 4.20 Lock hygiene

Rules to prevent deadlock:

1. Never hold two `async` locks simultaneously. If a command logically needs both `engine`
   and `layer_registry`, take the registry lock first, clone the profile out, release the
   lock, then take the engine lock.
2. Never hold any lock across a `.await` that could block indefinitely (e.g. the BLE scan).
3. `execute_action` for `PushLayer`/`PopLayer` acquires `engine` and `layer_registry` locks
   briefly; it must not be called while the event pump already holds `engine`. The event pump
   and `execute_action` must coordinate: collect outputs with engine locked, then release
   the lock **before** calling `execute_action`. (This is already how the event pump loop is
   written above.)

Document any function that acquires a lock with a `// LOCKING: acquires engine` comment.

### 4.21 Graceful shutdown and task 3.16

Register a `CloseRequested` window event handler in `lib.rs::run()`:

```rust
.on_window_event(|window, event| {
    if let tauri::WindowEvent::CloseRequested { .. } = event {
        let app = window.app_handle().clone();
        tauri::async_runtime::block_on(async move {
            if let Some(state) = app.try_state::<AppState>() {
                let mut manager = state.ble_manager.lock().await;
                for role in manager.connected_ids().cloned().collect::<Vec<_>>() {
                    let _ = manager.disconnect(&role).await;
                }
            }
        });
    }
})
```

This satisfies task 3.16 (deferred from Epic 3). The `disconnect` call in `BleManager`
already sends the exit controller mode packet before BLE-level disconnect.

---

## BLE status notifications — tap-ble changes

The `device-connected` and `device-disconnected` Tauri events (task 4.16) must be emitted
when the reconnect logic in `tap-ble` succeeds or when an unexpected disconnect is detected.
`tap-ble` has no access to the Tauri `AppHandle`, so a notification channel is used.

### `BleStatusEvent`

Add to `crates/tap-ble/src/manager.rs`:

```rust
#[derive(Debug, Clone)]
pub enum BleStatusEvent {
    Connected { device_id: DeviceId, address: BDAddr },
    Disconnected { device_id: DeviceId, address: BDAddr },
}
```

### `BleManager` changes

Add a `status_tx: broadcast::Sender<BleStatusEvent>` to `BleManager`. Capacity: 16.

- `connect()` sends `BleStatusEvent::Connected` after a successful connection.
- `disconnect()` sends `BleStatusEvent::Disconnected` after disconnecting.
- `reconnect_loop` in `tap_device.rs` (on reconnect success) sends `Connected`. This
  requires passing the sender into `TapDevice` (add `status_tx` param to `TapDevice::connect`
  and thread it through to `reconnect_loop`).
- `connection_monitor_task` on unexpected disconnect (before `reconnect_loop`) sends
  `Disconnected`.

Add to `BleManager`:

```rust
pub fn subscribe_status(&self) -> broadcast::Receiver<BleStatusEvent>
```

Called once during `run()` setup; the returned receiver is passed to a `ble_status_listener`
task.

### `ble_status_listener` task

Spawned in `lib.rs::run()` alongside the event pump:

```rust
tokio::spawn(ble_status_listener(app_handle.clone(), status_rx));
```

```rust
async fn ble_status_listener(
    app: tauri::AppHandle,
    mut rx: broadcast::Receiver<BleStatusEvent>,
) {
    while let Ok(event) = rx.recv().await {
        match event {
            BleStatusEvent::Connected { device_id, address } => {
                app.emit(events::DEVICE_CONNECTED, DeviceStatusPayload { ... }).ok();
            }
            BleStatusEvent::Disconnected { device_id, address } => {
                app.emit(events::DEVICE_DISCONNECTED, DeviceStatusPayload { ... }).ok();
            }
        }
    }
}
```

---

## New types (serde-serialisable DTOs)

These types are defined in `commands.rs` (or a `dto.rs` submodule if preferred). They derive
`serde::Serialize` and `serde::Deserialize` as needed for Tauri command boundaries.

### `TapDeviceInfoDto`

```rust
#[derive(Serialize)]
pub struct TapDeviceInfoDto {
    pub name: Option<String>,
    pub address: String,  // "AA:BB:CC:DD:EE:FF"
    pub rssi: Option<i16>,
}

impl From<&tap_ble::TapDeviceInfo> for TapDeviceInfoDto { ... }
```

### `ProfileSummary`

```rust
#[derive(Serialize)]
pub struct ProfileSummary {
    pub layer_id: String,
    pub name: String,
    pub kind: String,        // "single" or "dual"
    pub description: Option<String>,
}

impl From<&mapping_core::types::Profile> for ProfileSummary { ... }
```

### `EngineStateSnapshot`

```rust
#[derive(Serialize)]
pub struct EngineStateSnapshot {
    /// Layer IDs from bottom to top.
    pub layer_stack: Vec<String>,
    /// `layer_id` of the top (active) layer.
    pub active_layer_id: String,
    /// Current variable values (variable name → serialised value).
    pub variables: std::collections::HashMap<String, serde_json::Value>,
    /// Roles of currently connected BLE devices.
    pub connected_device_roles: Vec<String>,
    /// Whether debug mode is on.
    pub debug_mode: bool,
}
```

> **Note:** `ComboEngine` needs to expose a `variables()` accessor and a `debug_mode` field
> or getter for this to work. Confirm these exist in the current implementation; if not, add
> them as minor additions to `mapping-core` (which is a pre-approved change since it's
> additive).

---

## Testing strategy

### Unit tests

Tauri commands cannot easily be unit tested without a full Tauri context. For Epic 4, focus
integration tests on the state and helper logic:

| Subject | Approach |
| ------- | -------- |
| DTO conversion (`TapDeviceInfoDto`, `ProfileSummary`) | Unit tests in `commands.rs` — pure conversion logic, no Tauri needed |
| `execute_action` — keyboard simulation variants | Integration test with a real `enigo::Enigo` (may require a display; mark `#[ignore]` in CI if not available) |
| `LayerRegistry::reload()` error reporting (task 4.18 change) | Unit test in `mapping-core` using a temp dir with a malformed JSON file |

### Manual / smoke tests

The following are verified manually against the Svelte frontend (Epic 5 will add automated
UI tests):

- `scan_devices` returns discovered devices when a Tap is in range.
- `connect_device` → `get_engine_state` shows the device as connected.
- Tap a finger → `tap-event` received in Svelte dev console.
- `activate_profile` → engine responds to taps per the new profile.
- Close window → device exits controller mode (verified by device returning to text mode).

### CI note

BLE and display-dependent tests should be marked `#[ignore = "requires hardware or display"]`
so CI passes on headless runners.
