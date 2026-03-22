---
covers: Android BLE device management (post-Epic-15 bugfix / adaptation)
status: Draft — awaiting approval
last-updated: 2026-03-21
---

# Android BLE Device Management — Specification

## Table of contents

1. [Problem statement](#1-problem-statement)
2. [Design principles](#2-design-principles)
3. [User flows](#3-user-flows)
4. [Event types](#4-event-types)
5. [New Rust commands — `#[cfg(mobile)]`](#5-new-rust-commands--cfgmobile)
6. [AppState changes](#6-appstate-changes)
7. [Persistence](#7-persistence)
8. [android-bridge additions](#8-android-bridge-additions)
9. [Frontend — types](#9-frontend--types)
10. [Frontend — commands wrappers](#10-frontend--commands-wrappers)
11. [Frontend — devices page](#11-frontend--devices-page)
12. [Desktop commands — no change](#12-desktop-commands--no-change)
13. [Testing strategy](#13-testing-strategy)

---

## 1. Problem statement

The Devices page (`/devices`) calls `scan_devices` and `connect_device` — Rust commands that
are `#[cfg(not(mobile))]` and do not exist on Android. Pressing "Scan" produces
`"Command scan_devices not found"`.

Beyond the missing commands, the Android scanning and connection model is fundamentally
different from the desktop model:

| Aspect | Desktop | Android |
|--------|---------|---------|
| Scanning | Blocking 5-second call; returns a complete list | Event-based; `ble-device-found` events stream in as devices are discovered |
| Connection | Single call: BLE connect + role assignment + persistence | Two-step: BLE connect (Kotlin `BlePlugin`) + role assignment (Rust command) |
| Role concept | Assigned before connect | Assigned after BLE connect succeeds |
| Auto-reconnect | `tap-ble` crate manages reconnects | `BlePlugin` manages reconnects; Rust must restore role mapping on reconnect |

---

## 2. Design principles

1. **Standard mobile BLE feel.** Scan shows devices as they are found (streaming, not blocking).
   "Scan" becomes "Stop" while active. Devices appear immediately; the user does not wait
   for a fixed scan window to close.

2. **Connect first, assign role second.** Tapping "Connect" on a discovered device initiates the
   BLE connection immediately — no role picker blocks the connect. Role is assigned from the
   "Connected (unassigned)" list after the BLE connection succeeds. This matches how other
   mobile BLE apps behave.

3. **Role assignment is mandatory before the engine uses the device.** A device appears in the
   Rust `ComboEngine` only after the user assigns a role. Until then, tap bytes from the device
   are forwarded to the engine but the engine will ignore them (unknown `DeviceId`).

4. **Role persistence.** Once assigned, a device's role is saved to `android_devices.json`. On
   reconnect (auto or manual), the persisted role is restored automatically without user
   intervention.

5. **No changes to the desktop code path.** All new Rust commands are `#[cfg(mobile)]`. All
   existing desktop commands are unchanged.

6. **No changes to `deviceStore`.** The store's `onConnected` / `onDisconnected` interface
   consumes standard `DeviceStatusPayload` events, which the new Rust commands emit. The store
   is platform-agnostic.

---

## 3. User flows

### 3.1 First-time scan and connect

```
User taps "Scan"
  → frontend calls `plugin:ble|startScan`
  → BlePlugin starts scanning; emits `ble-device-found` events for each device found
  → frontend appends each device to discoveredDevices list (reactive, live updates)
  → "Scan" button becomes "Stop"
  → after 30 s BlePlugin auto-stops; frontend resets button to "Scan"
    (or user taps "Stop" earlier)

User taps "Connect" on a discovered device (e.g. "Tap Strap — AA:BB:CC:DD:EE:FF")
  → frontend calls `plugin:ble|connect { address }`
  → BlePlugin performs GATT setup (service discovery → notifications → controller mode)
  → BlePlugin emits `ble-device-connected { address, name }`
  → android-bridge catches the event, calls Rust `notify_android_device_connected(address, name)`
  → Rust looks up address in persisted android_devices — NOT found (first time)
  → Rust emits `ble-device-pending { address, name }` Tauri event
  → devices page is listening; device moves from discoveredDevices into pendingDevices list
  → "Connect" button disappears from the scan results for this address

User sees device in "Connected — assign role" section
  → role picker shows: [Solo] [Left] [Right]
  → user taps "Left"
  → frontend calls `assign_android_device(address, "left", name)` Rust command
  → Rust registers the device with the engine (DeviceId = address), saves to android_devices.json
  → Rust emits `device-connected { address, role: "left", name }` Tauri event
  → deviceStore.onConnected fires; device appears in deviceStore.connected
  → device disappears from pendingDevices; Devices page "Connected devices" section updates
```

### 3.2 Auto-reconnect on app restart or device out-of-range then back

```
App starts (or device comes back in range after BlePlugin reconnect)
  → BlePlugin auto-connects; emits `ble-device-connected { address, name }`
  → android-bridge calls `notify_android_device_connected(address, name)`
  → Rust looks up address in android_devices.json — FOUND with role "left"
  → Rust emits `device-connected { address, role: "left", name }` directly
  → deviceStore.onConnected fires; device appears in connected list immediately
  → no user action required
```

### 3.3 Disconnect

```
User taps "Disconnect" on a connected device
  → confirm dialog (reuse existing modal)
  → user confirms
  → frontend calls `plugin:ble|disconnect { address }`
  → BlePlugin disconnects GATT; sets userDisconnected flag (no auto-reconnect until next manual connect)
  → BlePlugin emits `ble-device-disconnected { address, reason: "user_request" }`
  → android-bridge calls `notify_android_device_disconnected(address)`
  → Rust looks up role from in-memory android_devices map; emits `device-disconnected { address, role }`
  → deviceStore.onDisconnected fires; device removed from connected list
  → Role is retained in android_devices.json so next manual connect auto-assigns it
```

### 3.4 Unexpected disconnect (out of range / BLE error)

```
BlePlugin exhausts reconnect attempts (or immediate disconnect)
  → BlePlugin emits `ble-device-disconnected { address, reason: "reconnect_failed" | "error" }`
  → android-bridge calls `notify_android_device_disconnected(address)`
  → same Rust path as §3.3 from "Rust looks up role" onwards
```

### 3.5 Role reassignment

```
User taps a different role on an already-connected device
  → frontend calls `reassign_android_device_role(address, new_role)`
  → Rust:
      1. looks up old role from in-memory map
      2. emits `device-disconnected { address, role: old_role }`
      3. updates in-memory map and android_devices.json with new_role
      4. emits `device-connected { address, role: new_role, name }`
  → deviceStore removes old entry, adds new entry
  → UI reflects the new role immediately
```

### 3.6 Second device (dual setup)

```
User connects a second Tap Strap using the same scan/connect flow
  → second device appears in "Connected — assign role"
  → role picker for second device shows only unoccupied roles
    (if first device is "left", only "right" is enabled; "solo" and "left" are greyed out)
  → user assigns "right"
  → dual profile becomes available
```

---

## 4. Event types

### 4.1 Events from Kotlin `BlePlugin` (existing — no change)

These are already emitted by `BlePlugin.kt`. This spec only adds listeners for them.

```typescript
// Emitted during an active scan for each device matching the Tap service UUID.
interface BleDeviceFoundPayload {
  address: string;       // MAC address
  name: string | null;   // device name (null if BLUETOOTH_CONNECT not granted)
  rssi: number;          // signal strength in dBm
}

// Emitted when GATT setup completes (service discovered, notifications enabled,
// controller mode entered). Same as desktop `device-connected` but without role.
interface BleDeviceConnectedPayload {
  address: string;
  name: string | null;
}

// Emitted on disconnect (user-initiated or error/timeout).
interface BleDeviceDisconnectedPayload {
  address: string;
  reason: "user_request" | "reconnect_failed" | string;
}
```

### 4.2 New event from Rust: `ble-device-pending`

Emitted by `notify_android_device_connected` when the device is BLE-connected but has no
persisted role. Consumed by the devices page to show the role-assignment UI.

```typescript
interface BleDevicePendingPayload {
  address: string;
  name: string | null;
}
```

Event name constant (Rust): `"ble-device-pending"`

### 4.3 Existing events — unchanged

`device-connected` and `device-disconnected` continue to use `DeviceStatusPayload`:

```typescript
interface DeviceStatusPayload {
  address: string;
  role: string;        // "solo" | "left" | "right"
  name: string | null;
}
```

These are emitted by the new Rust commands exactly as the desktop does, so `events.ts`
and `deviceStore` require no changes.

---

## 5. New Rust commands — `#[cfg(mobile)]`

All commands in this section are added to `apps/desktop/src-tauri/src/commands.rs` under a
`#[cfg(mobile)]` block. Desktop commands in the same file are unchanged.

### 5.1 `notify_android_device_connected`

Called by the android-bridge when `ble-device-connected` fires from BlePlugin.

```rust
#[cfg(mobile)]
#[tauri::command]
pub async fn notify_android_device_connected(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    address: String,
    name: Option<String>,
) -> Result<(), String>
```

Behaviour:
1. Lock `state.android_devices`.
2. If `address` is in the map (persisted role found):
   - Emit `device-connected` with `{ address, role, name }`.
   - Return `Ok`.
3. If `address` is not in the map:
   - Emit `ble-device-pending` with `{ address, name }`.
   - Return `Ok`.

Does **not** persist anything (role is unknown at this point).

### 5.2 `assign_android_device`

Called by the devices page when the user selects a role for a pending device.

```rust
#[cfg(mobile)]
#[tauri::command]
pub async fn assign_android_device(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    address: String,
    role: String,
    name: Option<String>,
) -> Result<(), String>
```

Behaviour:
1. Validate `role` is one of `"solo"`, `"left"`, `"right"`.
2. Build an `AndroidDeviceRecord { address, role, name }`.
3. Insert into `state.android_devices` (in-memory map).
4. Persist `android_devices.json` (see §7).
5. Emit `device-connected` with `{ address, role, name }`.
6. Return `Ok`.

### 5.3 `notify_android_device_disconnected`

Called by the android-bridge when `ble-device-disconnected` fires from BlePlugin.

```rust
#[cfg(mobile)]
#[tauri::command]
pub async fn notify_android_device_disconnected(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    address: String,
) -> Result<(), String>
```

Behaviour:
1. Look up `address` in `state.android_devices`.
2. If found: emit `device-disconnected` with `{ address, role, name }`.
3. If not found: log a warning; return `Ok` (may be a device that was never assigned a role).
4. Role record is **not removed** from `android_devices` — it is retained for the next
   reconnect. This is intentional: BlePlugin may reconnect automatically.

### 5.4 `reassign_android_device_role`

Called when the user changes the role of an already-connected device.

```rust
#[cfg(mobile)]
#[tauri::command]
pub async fn reassign_android_device_role(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    address: String,
    new_role: String,
) -> Result<(), String>
```

Behaviour:
1. Look up `address` in `state.android_devices` — error if not found.
2. Emit `device-disconnected` with old role.
3. Update in-memory record's role.
4. Persist `android_devices.json`.
5. Emit `device-connected` with new role.

### 5.5 Command registration

Register all four commands in `lib.rs` `invoke_handler` under the existing mobile branch:

```rust
#[cfg(mobile)]
{
    tauri::generate_handler![
        // existing mobile commands …
        commands::notify_android_device_connected,
        commands::assign_android_device,
        commands::notify_android_device_disconnected,
        commands::reassign_android_device_role,
    ]
}
```

---

## 6. AppState changes

Add a mobile-only field to `AppState` in `state.rs`:

```rust
#[cfg(mobile)]
pub android_devices: tokio::sync::Mutex<std::collections::HashMap<String, AndroidDeviceRecord>>,
```

`AndroidDeviceRecord` is a serialisable struct defined in `commands.rs` (or `state.rs`):

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AndroidDeviceRecord {
    pub address: String,
    pub role: String,
    pub name: Option<String>,
}
```

In `build_app_state()` (in `state.rs`), initialise the field by loading from
`android_devices.json` (see §7). If the file does not exist, initialise to an empty map.

```rust
#[cfg(mobile)]
android_devices: tokio::sync::Mutex::new(load_android_devices(&data_dir)),
```

---

## 7. Persistence

### 7.1 File location

`android_devices.json` is stored in the same directory as `profiles.json` — the app data
directory returned by `platform::data_dir()` on Android
(`/data/data/com.mapxr.app/files/` or equivalent).

### 7.2 Format

```json
[
  { "address": "AA:BB:CC:DD:EE:FF", "role": "left",  "name": "Tap Strap" },
  { "address": "11:22:33:44:55:66", "role": "right", "name": null }
]
```

A flat JSON array of `AndroidDeviceRecord` objects. There is no schema version field —
the format is simple enough that forward-compatibility is not a concern.

### 7.3 Read/write helpers

Add two private `#[cfg(mobile)]` functions to `commands.rs` (or extract to a small
`android_devices.rs` module if the file grows):

```rust
fn load_android_devices(data_dir: &Path) -> HashMap<String, AndroidDeviceRecord>
fn save_android_devices(data_dir: &Path, devices: &HashMap<String, AndroidDeviceRecord>)
    -> Result<(), anyhow::Error>
```

`load_android_devices` returns an empty map if the file is absent or unreadable (non-fatal).
`save_android_devices` overwrites the file atomically (write to `.tmp`, then rename).

---

## 8. android-bridge additions

`apps/desktop/src/lib/android-bridge.ts` gains two new listeners in `startAndroidBridge()`:

```typescript
// When BlePlugin reports a device connected, notify the Rust engine.
const unlistenConnected = await listen<BleDeviceConnectedPayload>(
  "ble-device-connected",
  async ({ payload }) => {
    try {
      await invoke("notify_android_device_connected", {
        address: payload.address,
        name: payload.name ?? null,
      });
    } catch (err) {
      logger.warn(`android-bridge: notify_android_device_connected failed: ${err}`);
    }
  },
);

// When BlePlugin reports a device disconnected, notify the Rust engine.
const unlistenDisconnected = await listen<BleDeviceDisconnectedPayload>(
  "ble-device-disconnected",
  async ({ payload }) => {
    try {
      await invoke("notify_android_device_disconnected", { address: payload.address });
    } catch (err) {
      logger.warn(`android-bridge: notify_android_device_disconnected failed: ${err}`);
    }
  },
);
```

Both unlisteners are included in the returned cleanup function.

---

## 9. Frontend — types

Add to `apps/desktop/src/lib/types.ts`:

```typescript
/** A device found during an Android BLE scan. */
export interface BleDeviceFoundPayload {
  address: string;
  name: string | null;
  rssi: number;
}

/** Emitted by Rust when a BLE-connected device has no persisted role. */
export interface BleDevicePendingPayload {
  address: string;
  name: string | null;
}

/** Payload from BlePlugin when a device connects or disconnects. */
export interface BleDeviceConnectedPayload {
  address: string;
  name: string | null;
}

export interface BleDeviceDisconnectedPayload {
  address: string;
  reason: string;
}
```

---

## 10. Frontend — commands wrappers

Add to `apps/desktop/src/lib/commands.ts`:

```typescript
/** [Android] Assign a role to a BLE-connected device with no persisted role. */
export async function assignAndroidDevice(
  address: string,
  role: string,
  name: string | null,
): Promise<void> {
  return invoke("assign_android_device", { address, role, name });
}

/** [Android] Reassign the role of an already-connected device. */
export async function reassignAndroidDeviceRole(
  address: string,
  newRole: string,
): Promise<void> {
  return invoke("reassign_android_device_role", { address, newRole: new_role });
}
```

> Note: Tauri snake_cases parameter names in invoke payloads by default. Verify the
> parameter name casing matches what the Rust command expects.

Also add wrappers for the two BlePlugin commands:

```typescript
/** [Android] Start BLE scanning. Devices are streamed as `ble-device-found` events. */
export async function startBleScan(): Promise<void> {
  return invoke("plugin:ble|startScan");
}

/** [Android] Stop an active BLE scan. */
export async function stopBleScan(): Promise<void> {
  return invoke("plugin:ble|stopScan");
}

/** [Android] Connect to a Tap device by MAC address. */
export async function bleConnect(address: string): Promise<void> {
  return invoke("plugin:ble|connect", { address });
}

/** [Android] Disconnect from a Tap device by MAC address. */
export async function bleDisconnect(address: string): Promise<void> {
  return invoke("plugin:ble|disconnect", { address });
}
```

---

## 11. Frontend — devices page

`apps/desktop/src/routes/devices/+page.svelte` gains a platform branch. The desktop
code path is **unchanged**. The mobile code path replaces the scan/connect/disconnect
sections.

### 11.1 Platform detection

At the top of `<script>`:

```typescript
import { getPlatform } from "$lib/commands";

let isAndroid = $state(false);
onMount(async () => {
  isAndroid = (await getPlatform()) === "android";
});
```

### 11.2 Android scan state

```typescript
// Android scan state (distinct from desktop discoveredDevices)
let bleScanning = $state(false);
let bleDiscovered = $state<BleDeviceFoundPayload[]>([]);
let bleScanError = $state<string | null>(null);
let bleUnlistenFound: (() => void) | null = null;

// Devices BLE-connected but not yet assigned a role
let pendingDevices = $state<BleDevicePendingPayload[]>([]);
let blePendingUnlisten: (() => void) | null = null;
```

### 11.3 Scan lifecycle

```typescript
async function handleAndroidScan() {
  bleScanning = true;
  bleScanError = null;
  bleDiscovered = [];

  // Listen for individual device events.
  bleUnlistenFound = await listen<BleDeviceFoundPayload>("ble-device-found", ({ payload }) => {
    // Deduplicate by address; update rssi if already seen.
    const idx = bleDiscovered.findIndex((d) => d.address === payload.address);
    if (idx === -1) {
      bleDiscovered = [...bleDiscovered, payload];
    } else {
      bleDiscovered = bleDiscovered.map((d, i) => (i === idx ? payload : d));
    }
  });

  try {
    await startBleScan();
  } catch (e) {
    bleScanError = e instanceof Error ? e.message : String(e);
    bleScanning = false;
    bleUnlistenFound?.();
  }

  // Auto-stop indicator: BlePlugin stops after 30 s; mirror that in the UI.
  setTimeout(() => {
    if (bleScanning) {
      bleScanning = false;
      bleUnlistenFound?.();
    }
  }, 30_000);
}

async function handleAndroidStopScan() {
  bleScanning = false;
  bleUnlistenFound?.();
  await stopBleScan().catch(() => {});
}
```

### 11.4 `ble-device-pending` listener

Set up in `onMount`, torn down in cleanup:

```typescript
blePendingUnlisten = await listen<BleDevicePendingPayload>("ble-device-pending", ({ payload }) => {
  // Avoid duplicates (e.g. rapid reconnects).
  if (!pendingDevices.some((d) => d.address === payload.address)) {
    pendingDevices = [...pendingDevices, payload];
  }
  // Remove from scan results (it has connected).
  bleDiscovered = bleDiscovered.filter((d) => d.address !== payload.address);
});
```

When `device-connected` fires (via `events.ts` → `deviceStore`), remove the device from
`pendingDevices`:

```typescript
// Reactive: remove from pending when deviceStore picks it up.
let connectedAddresses = $derived(new Set(deviceStore.connected.map((d) => d.address)));
$effect(() => {
  pendingDevices = pendingDevices.filter((d) => !connectedAddresses.has(d.address));
});
```

### 11.5 Android connect flow

```typescript
let androidConnectingAddress = $state<string | null>(null);
let androidConnectError = $state<string | null>(null);

async function handleAndroidConnect(address: string, name: string | null) {
  androidConnectingAddress = address;
  androidConnectError = null;
  try {
    await bleConnect(address);
    deviceStore.setName(address, name);
    // Device moves to pendingDevices via `ble-device-pending` event.
  } catch (e) {
    androidConnectError = e instanceof Error ? e.message : String(e);
  } finally {
    androidConnectingAddress = null;
  }
}
```

### 11.6 Role assignment

```typescript
let pendingRoles = $state<Record<string, string>>({});

function selectPendingRole(address: string, role: string) {
  pendingRoles = { ...pendingRoles, [address]: role };
}

async function handleAssignRole(address: string, name: string | null) {
  const role = pendingRoles[address];
  if (!role) return;
  try {
    await assignAndroidDevice(address, role, name);
    // deviceStore.onConnected fires via device-connected event; pendingDevices
    // reactively cleared by the $effect above.
    const { [address]: _, ...rest } = pendingRoles;
    pendingRoles = rest;
  } catch (e) {
    logger.error("assignAndroidDevice failed", e);
  }
}
```

Role button disabled logic: a role is disabled if it is already occupied by a device in
`deviceStore.connected`.

```typescript
function isRoleTaken(role: string): boolean {
  return deviceStore.connected.some((d) => d.role === role);
}
```

### 11.7 Android disconnect

```typescript
async function handleAndroidDisconnect(address: string) {
  try {
    await bleDisconnect(address);
    // `device-disconnected` fires via ble-device-disconnected → android-bridge →
    // notify_android_device_disconnected → device-disconnected Tauri event →
    // events.ts → deviceStore.onDisconnected.
  } catch (e) {
    logger.error("bleDisconnect failed", e);
  }
}
```

Disconnect confirmation modal reuses the existing modal; it passes `address` instead of
`role` for the Android path.

### 11.8 Android role reassignment

```typescript
async function handleAndroidReassign(address: string, newRole: string) {
  try {
    await reassignAndroidDeviceRole(address, newRole);
    // device-disconnected then device-connected events update deviceStore automatically.
  } catch (e) {
    logger.error("reassignAndroidDeviceRole failed", e);
  }
}
```

### 11.9 Template structure (Android branch)

The template uses `{#if isAndroid}` to render the Android-specific sections.

**Scan section (Android):**
```
[Scan / Stop button]
[Spinner while scanning]
[Error alert if bleScanError]
[Table of bleDiscovered devices — each row has name, address, RSSI badge, [Connect] button]
```

**Pending role assignment section (Android):**
Only shown if `pendingDevices.length > 0`.
```
[Heading: "Assign role"]
[For each pendingDevice:
  - Device name / address
  - [Solo] [Left] [Right] role buttons (disabled if role taken)
  - [Assign] button (disabled until a role is selected)]
```

**Connected devices section:**
Shared between desktop and Android (already rendered from `deviceStore.connected`).
The reassign role buttons on Android call `handleAndroidReassign` instead of
`handleReassign`.
The disconnect button on Android calls `handleAndroidDisconnect(device.address)` instead of
`handleDisconnect(device.role)`.

### 11.10 Signal strength badge

For scan results on Android, display RSSI using the same `rssiLabel` / `rssiClass`
helpers already defined in the desktop devices page. The threshold values are identical.

---

## 12. Desktop commands — no change

`scan_devices`, `connect_device`, `disconnect_device`, `reassign_device_role` in
`commands.rs` are unchanged. The desktop devices page code path is unchanged.

---

## 13. Testing strategy

### Manual tests (require physical hardware)

| # | Scenario | Expected |
|---|----------|----------|
| M1 | Tap "Scan" — Tap device powered on and advertising | Device appears within ~5 s; RSSI badge shown |
| M2 | Tap "Stop" during active scan | Scan stops; discovered list retained |
| M3 | Scan auto-timeout (wait 30 s) | "Scan" button resets; discovered list retained |
| M4 | Connect a device | Device moves from scan list to "Assign role" section |
| M5 | Assign role "Solo" | Device appears in "Connected devices"; sidebar finger visualiser active |
| M6 | Assign role "Left", then connect second device and assign "Right" | Both devices shown; Right role button disabled on first device |
| M7 | Role reassignment | Old role vacated; new role active; engine routes taps to new role |
| M8 | Disconnect | Device removed from connected list; BLE disconnects; no auto-reconnect |
| M9 | Kill app, restart — previously connected device re-advertises | Device auto-reconnects, role restored; no user action required |
| M10 | Device goes out of range; returns | BlePlugin reconnects; role restored automatically |
| M11 | Connect device on Android; use Tap Strap | Tap events fire correctly (regression test for engine routing) |

### Unit tests (Rust)

- `notify_android_device_connected_known_device_emits_device_connected` — mock AppState
  with pre-populated `android_devices`; verify `device-connected` event emitted.
- `notify_android_device_connected_unknown_device_emits_ble_device_pending` — empty map;
  verify `ble-device-pending` emitted.
- `assign_android_device_saves_and_emits` — verify record written to map and
  `device-connected` emitted.
- `notify_android_device_disconnected_known_device_emits_device_disconnected` — verify event.
- `notify_android_device_disconnected_unknown_device_logs_warning_no_panic` — verify no crash.
- `reassign_android_device_role_emits_both_events` — verify `device-disconnected` old role
  then `device-connected` new role.
- `save_load_android_devices_round_trip` — write to temp dir, reload, verify equality.
