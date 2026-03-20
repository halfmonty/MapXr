---
covers: Epic 3 (BLE layer — tap-ble crate)
status: Approved and fully implemented
last-updated: 2026-03-19
---

# tap-ble — BLE layer specification (Epic 3)

## Table of contents

1. [Overview](#overview)
2. [Dependencies](#dependencies)
3. [GATT protocol reference](#gatt-protocol-reference)
4. [Module structure](#module-structure)
5. [Device discovery](#device-discovery)
6. [Connection and GATT setup](#connection-and-gatt-setup)
7. [Controller mode](#controller-mode)
8. [Tap data stream](#tap-data-stream)
9. [Device registry](#device-registry)
10. [Error types](#error-types)
11. [Integration with mapping-core](#integration-with-mapping-core)
12. [Reconnection policy](#reconnection-policy)
13. [Shutdown and cleanup](#shutdown-and-cleanup)
14. [Testing strategy](#testing-strategy)

---

## Overview

The `tap-ble` crate is the BLE abstraction layer for mapxr. It owns the lifecycle of one or two
Tap wearable devices: scanning, connecting, entering controller mode, streaming raw tap events,
and reconnecting on dropout. Its only output to the rest of the system is a stream of `RawTapEvent`
structs (defined in `mapping-core`) distributed via a `tokio::sync::broadcast` channel.

`tap-ble` has no knowledge of profiles, mappings, or UI state. It is a pure hardware interface.

---

## Dependencies

The following dependencies are added to `crates/tap-ble/Cargo.toml`:

| Crate | Purpose |
| ----- | ------- |
| `btleplug` | BLE adapter, scanning, GATT client |
| `tokio` (already workspace) | Async runtime, timers, channels |
| `thiserror` (already workspace) | Error types |
| `uuid` | UUID constants — `btleplug 0.12` does not re-export `Uuid`, so a direct dep is required |
| `serde` + `serde_json` | `DeviceRegistry` persistence |

`btleplug` is pre-approved per `CLAUDE.md`. `uuid` is required as a direct dependency —
`btleplug 0.12` uses `uuid::Uuid` privately and does not re-export it.

---

## GATT protocol reference

All constants are sourced from `docs/reference/api-doc.txt` and `docs/reference/windows-sdk-guid-reference.txt`.

### Services

| Name | UUID | Notes |
| ---- | ---- | ----- |
| Tap proprietary | `C3FF0001-1D8B-40FD-A56F-C7BD5D0F3370` | Used as scan filter; main API service |
| NUS (Nordic UART) | `6E400001-B5A3-F393-E0A9-E50E24DCCA9E` | Support service; carries the command channel |
| Device Information | `0000180A-0000-1000-8000-00805F9B34FB` | Standard BLE DIS service |
| Battery | Standard BLE Battery Service UUID | Standard BLE BAS service |

### Characteristics

| Name | UUID | Properties | Used by mapxr? |
| ---- | ---- | ---------- | -------------- |
| Tap data | `C3FF0005-1D8B-40FD-A56F-C7BD5D0F3370` | Notify | Yes — tap events |
| Mouse data | `C3FF0006-1D8B-40FD-A56F-C7BD5D0F3370` | Notify | No (Text mode only) |
| Air gestures data | `C3FF000A-1D8B-40FD-A56F-C7BD5D0F3370` | Notify | No (out of scope) |
| UI commands | `C3FF0009-1D8B-40FD-A56F-C7BD5D0F3370` | — | No |
| NUS RX | `6E400002-B5A3-F393-E0A9-E50E24DCCA9E` | Write | Yes — controller mode commands |
| NUS TX | `6E400003-B5A3-F393-E0A9-E50E24DCCA9E` | Notify | No |
| FW version | `00002A26-0000-1000-8000-00805F9B34FB` | Read | Optional (device info) |
| Battery level | Standard BLE Battery Level UUID | Read | Optional (status display) |

> Raw sensor mode (five 3-axis accelerometers at 200 Hz + IMU at 208 Hz, available from FW
> 2.3.27) is documented in `docs/reference/raw-sensor-mode.txt`. It is **out of scope for
> Epic 3** but is the foundation for stretch goal S.2. The protocol and characteristic UUIDs
> required for raw sensor mode should not be wired up in this epic.

### Pairing requirement

The device must be **bonded** (paired), not merely connected. Per the API doc: "For proper usage
with the Tap, the Tap must be also paired (bonded) and not just connected." mapxr relies on the
OS pairing flow; `btleplug` does not expose explicit pairing APIs on all platforms. Document any
platform-specific pairing caveats discovered during implementation.

### ATT MTU

The API doc notes an ATT MTU of 185 is required for certain operations (specifics TBA by Tap).
`btleplug` negotiates MTU automatically on connection; no explicit MTU negotiation is needed in
application code, but log the negotiated MTU at `DEBUG` level for diagnostics.

### Controller mode protocol

| Packet | Byte 0 | Byte 1 | Byte 2 | Byte 3 |
| ------ | ------ | ------ | ------ | ------ |
| Enter controller mode | `0x03` | `0x0C` | `0x00` | `0x01` |
| Exit controller mode  | `0x03` | `0x0C` | `0x00` | `0x00` |

Written to **NUS RX** (`6E400002`). The device must receive the enter packet before it emits tap
data notifications. The keepalive interval is **10 seconds** — the enter packet must be re-sent
every 10 s to prevent the device from returning to Text mode. Always send the exit packet before
disconnect or app shutdown.

> Note from API doc: during Controller mode, multi-taps and Switch & Shift behaviour are not
> supported. This is expected — mapxr implements its own double/triple-tap logic in `mapping-core`.

### Tap data packet format

Each notification on characteristic `C3FF0005` carries 3 bytes (all multi-byte fields are
little-endian per the API doc):

```
byte 0:    tap_code  (u8)   — bitmask; bit 0 = thumb (LSB), bit 4 = pinky
bytes 1–2: interval  (u16 LE) — ms since previous tap, saturated at 65535
bits 5–7:  unused
```

Bit-to-finger mapping (from `api-doc.txt`):

| Bit | Finger |
| --- | ------ |
| 0 (LSB) | Thumb |
| 1 | Index |
| 2 | Middle |
| 3 | Ring |
| 4 | Pinky |
| 5–7 | Unused |

This mapping is hardware-normalised: left-hand and right-hand devices use the same bit positions.
Example: `tap_code = 0x1F` (31) means all five fingers; `tap_code = 0x03` means thumb + index.

The `interval` field is informational. `RawTapEvent.received_at` is stamped with `Instant::now()`
at the point the notification callback fires, giving wall-clock accuracy independent of the
device's internal clock.

---

## Module structure

```
crates/tap-ble/src/
├── lib.rs              — re-exports; public API surface
├── error.rs            — BleError enum
├── device_info.rs      — TapDeviceInfo struct (name, address, rssi)
├── scanner.rs          — discover_devices(), scan loop
├── tap_device.rs       — TapDevice: connect, controller mode, notification loop
├── device_registry.rs  — DeviceRegistry: DeviceId → address persistence
└── manager.rs          — BleManager: top-level coordinator used by Tauri layer
```

---

## Device discovery

### `TapDeviceInfo`

```rust
pub struct TapDeviceInfo {
    pub name: Option<String>,
    pub address: BDAddr,          // btleplug re-export
    pub rssi: Option<i16>,
}
```

### `discover_devices`

Signature:

```rust
pub async fn discover_devices(timeout_ms: u64) -> Result<Vec<TapDeviceInfo>, BleError>
```

Behaviour:
1. Obtain the default BLE adapter. Return `BleError::AdapterNotFound` if none available.
2. Start a scan filtered to `TAP_SERVICE_UUID`.
3. Collect discovered devices for `timeout_ms` milliseconds (recommended default: 5000).
4. Stop the scan.
5. Deduplicate by address (a device may appear in multiple `DeviceDiscovered` events).
6. Return the list sorted by RSSI descending (strongest signal first), with `None` RSSI last.

Scan result deduplication: maintain a `HashMap<BDAddr, TapDeviceInfo>` and update the entry
on each event (later events have fresher RSSI).

---

## Connection and GATT setup

### `TapDevice`

`TapDevice` represents a single connected Tap device. It is created by `BleManager` after a
successful connection.

```rust
pub struct TapDevice {
    // internal: btleplug Peripheral + discovered characteristic handles
}
```

### Connection flow

1. Call `peripheral.connect()`. This triggers OS-level bonding/pairing if the device has not
   been paired before. The OS pairing UI is out of `tap-ble`'s control.
2. On success, call `peripheral.discover_services()`.
3. Locate the tap data characteristic (`C3FF0005`) and the NUS RX characteristic (`6E400002`).
   If either is missing, return `BleError::MissingCharacteristic { uuid }`.
4. Log the negotiated ATT MTU at `DEBUG` level if `btleplug` exposes it.
5. Subscribe to notifications on the tap data characteristic.
6. Enter controller mode (see §Controller mode).

### Already-connected device

If `peripheral.is_connected()` is already true when `connect()` is called, skip the
`peripheral.connect()` call but still run service discovery and mode entry (the device may
have lost mode state if the keepalive lapsed).

### Already-bonded-to-another-host

Per the API doc: if the device is currently connected to another peer, it cannot be connected
to a second host — it must be disconnected from the active peer first. `btleplug` surfaces this
as an OS-level error. Return `BleError::ConnectionRefused { address, reason }` and surface a
human-readable message to the caller (e.g. "Device is already connected to another host — disable
Bluetooth on the other device and retry").

---

## Controller mode

### Entry

Write `[0x03, 0x0C, 0x00, 0x01]` to the NUS RX characteristic with `write_without_response`.

### Keepalive timer

Spawn a `tokio` task that loops: sleep 10 seconds, re-send the enter packet. The task holds a
weak reference to the characteristic handle and exits if the `TapDevice` is dropped.

Concretely: use a `CancellationToken` (from `tokio-util`) — or a `tokio::sync::watch` channel
with a bool — to signal the keepalive task to stop. Check dependency policy before adding
`tokio-util`; if not already present, use a `oneshot` channel for cancellation instead.

### Exit

Write `[0x03, 0x0C, 0x00, 0x00]` to the NUS RX characteristic. This must be called:
- On `TapDevice::disconnect()`.
- On app shutdown (via a Tauri `on_window_event` hook registered in Epic 4).
- In the `Drop` impl of `TapDevice` as a best-effort fallback (using `block_in_place` or by
  spawning a detached task — document the tradeoff in a `// SAFETY:` or `// NOTE:` comment).

---

## Tap data stream

### Notification handler

After subscribing to `C3FF0005`, spawn a tokio task reading from
`peripheral.notifications()` (a `Stream<Item = ValueNotification>`).

For each notification on `C3FF0005`:
1. Parse the 3-byte packet. If len < 1, log a warning and skip (do not panic).
2. Construct:
   ```rust
   RawTapEvent {
       device_id: self.device_id.clone(),
       tap_code: packet[0],
       received_at: Instant::now(),
   }
   ```
3. Send on the broadcast channel. A `SendError` (no receivers) is not an error — drop silently.

### Broadcast channel

`BleManager` owns a `tokio::sync::broadcast::Sender<RawTapEvent>`. The channel capacity is
**64** events (configurable via a const, not hardcoded in the send call). Callers obtain a
receiver by calling `BleManager::subscribe()`.

---

## Device registry

### Purpose

Maps logical device roles (`"left"`, `"right"`, `"solo"`) to BLE hardware addresses, so the
app can reconnect to the same physical devices across sessions without re-scanning.

### `DeviceId`

Re-exported from `mapping-core` (already defined there as a newtype over `String`).
Valid values: `"left"`, `"right"`, `"solo"`.

### `DeviceRegistry`

```rust
pub struct DeviceRegistry {
    entries: HashMap<DeviceId, BDAddr>,
}

impl DeviceRegistry {
    pub fn load(path: &Path) -> Result<Self, BleError>;
    pub fn save(&self, path: &Path) -> Result<(), BleError>;
    pub fn assign(&mut self, device_id: DeviceId, address: BDAddr);
    pub fn address_for(&self, device_id: &DeviceId) -> Option<BDAddr>;
    pub fn remove(&mut self, device_id: &DeviceId);
}
```

Persistence format: `devices.json` stored alongside the profiles directory (one level up, or in
the same config root — to be confirmed with the user; the path is passed in from `src-tauri`).

```json
{
  "version": 1,
  "devices": {
    "solo":  "AA:BB:CC:DD:EE:FF",
    "left":  "11:22:33:44:55:66",
    "right": "77:88:99:AA:BB:CC"
  }
}
```

`BDAddr` serialises as a colon-separated hex string (uppercase). Implement custom
`Serialize`/`Deserialize` for `BDAddr` if `btleplug` does not provide it.

### Role validation

On engine start, `BleManager::check_roles(profile: &Profile)` warns (logs, does not error) if
the loaded profile's `kind` is `Dual` but only one device (or no devices) are registered.

---

## Error types

```rust
#[derive(Debug, thiserror::Error)]
pub enum BleError {
    #[error("no BLE adapter found on this system")]
    AdapterNotFound,

    #[error("characteristic {uuid} not found on device {address}")]
    MissingCharacteristic { uuid: String, address: String },

    #[error("connection refused to {address}: {reason}")]
    ConnectionRefused { address: String, reason: String },

    #[error("device {address} disconnected unexpectedly")]
    UnexpectedDisconnect { address: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("device {address} not found in scan results — run a scan first")]
    DeviceNotFound { address: String },

    #[error("BLE driver error: {0}")]
    Btleplug(#[from] btleplug::Error),
}
```

---

## Integration with mapping-core

`tap-ble` depends on `mapping-core` for `RawTapEvent` and `DeviceId`. The dependency direction
is: `tap-ble` → `mapping-core`. `mapping-core` has no knowledge of `tap-ble`.

The Tauri layer (Epic 4) holds both a `BleManager` and a `ComboEngine`. It calls
`ble_manager.subscribe()` to get a `broadcast::Receiver<RawTapEvent>` and feeds each event
into `engine.push_event(event, Instant::now())` in a dedicated tokio task.

---

## Reconnection policy

On `UnexpectedDisconnect`:
1. Cancel the keepalive task.
2. Begin a reconnect loop with exponential backoff: 1 s, 2 s, 4 s, 8 s, … capped at 60 s.
3. Each attempt calls the full connection flow (connect → service discovery → controller mode).
4. On success, resume the notification listener and keepalive.
5. Emit a `device-connected` event (wired up in Epic 4) so the UI can update.
6. Maximum retry count: unlimited (retry until the device is found or `disconnect()` is called
   explicitly).

Cancellation: `TapDevice::disconnect()` sets a flag that the reconnect loop checks; on seeing
it, the loop exits without further attempts.

---

## Shutdown and cleanup

### Ordered shutdown sequence

1. For each connected `TapDevice`, call `exit_controller_mode()`.
2. Call `btleplug` disconnect on each peripheral.
3. Drop the broadcast sender (receivers will observe `RecvError::Closed`).

### Drop guard

`TapDevice` implements `Drop`. In the drop impl, make a best-effort attempt to send the exit
controller mode packet synchronously. Use `Handle::current().block_on(...)` if a runtime is
available, or spawn a detached task. Add a `// NOTE: best-effort drop; prefer explicit disconnect`
comment.

### Test for drop guard (task 3.17)

Spawn a `TapDevice` with a mock peripheral. Drop it without calling `disconnect()`. Assert that
the exit packet was written to the NUS RX characteristic.

---

## Testing strategy

### Unit tests

| Subject | Approach |
| ------- | -------- |
| Packet parser (task 3.19, 3.22) | Pure function; no hardware needed. Test all 32 valid codes, minimum/maximum interval values, and undersized packets. |
| `DeviceRegistry` load/save | Use `tempfile::tempdir()`; test round-trip, missing file (returns empty registry), and malformed JSON. |
| UUID filter constant | Static assertion that the constant parses without panic. |

### Integration tests (require hardware or mock)

BLE connection, notification streaming, and controller mode entry/exit cannot be meaningfully
tested without a real device or a mock BLE stack. For task 3.6 (UUID filter test), a mock
peripheral that advertises the known service UUID is acceptable. `btleplug` does not ship a
built-in mock; implement a minimal in-process mock using `btleplug::api` trait objects, or
document that task 3.6 is a manual test performed against a physical device.

The spec acknowledges that hardware-dependent tests may be marked `#[ignore]` with a comment
explaining the requirement (e.g. `// requires physical Tap device`).
