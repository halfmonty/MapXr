---
covers: Epic 10 (device renaming)
status: Draft — awaiting approval
last-updated: 2026-03-19
---

# Device renaming — specification (Epic 10)

## Table of contents

1. [Overview](#overview)
2. [GATT characteristics](#gatt-characteristics)
3. [Write protocol](#write-protocol)
4. [Constraints and validation](#constraints-and-validation)
5. [tap-ble API](#tap-ble-api)
6. [Tauri command](#tauri-command)
7. [Frontend](#frontend)
8. [Error handling](#error-handling)
9. [Testing strategy](#testing-strategy)

---

## Overview

Users can assign a custom friendly name to a connected Tap device. The name is written directly
to the device over BLE and persists in device flash across power cycles and reconnections.

The name change takes effect on the **next reconnection cycle** — the device must disconnect and
re-advertise before the new name appears in scan results. The UI must communicate this to the user.

---

## GATT characteristics

Two writable characteristics hold the device name. Both are R/W on TapXR (FW 3.4.0) and
Tap Strap 2 (FW 2.7.0), confirmed from `docs/reference/gatt-probe-output.txt`.

| UUID | Service | Label | Properties |
|------|---------|-------|------------|
| `00002A00` | `00001800` Generic Access Profile | GAP Device Name | R / W |
| `C3FF0003` | `C3FF0001` Tap Proprietary | Tap Device Name (copy) | R / W |

The two characteristics mirror each other — both read back the same value on both devices tested.
The gatt reference notes that writing to `C3FF0003` "presumably updates both"; this is unverified.

**Write strategy:** write to `C3FF0003` only.

`00002A00` (GAP Device Name) was found to reject writes with "Operation Not Authorized"
at runtime despite advertising the `WRITE` property in the characteristic flags. `C3FF0003`
alone is sufficient — the device re-advertises the updated name on the next reconnect.

---

## Write protocol

The name value is encoded as **raw UTF-8 bytes** with no null terminator, no length prefix, and
no other framing. This matches the format observed on both devices (e.g. `TapXR_A036320` is
13 bytes of ASCII, written identically to both characteristics).

`C3FF0003` is written using `btleplug` `write_characteristic` with `WriteType::WithResponse`
(the characteristic lists `WRITE`, not just `WRITE_WITHOUT_RESPONSE`).

`00002A00` is **not** written. Although it lists `WRITE` in its properties, the device
rejects writes with "Operation Not Authorized" at runtime (confirmed on TapXR FW 3.4.0).

### Observed values for reference

| Device | Characteristic | Hex | UTF-8 |
|--------|---------------|-----|-------|
| TapXR | `00002A00` / `C3FF0003` | `54 61 70 58 52 5F 41 30 33 36 33 32 30` | `TapXR_A036320` |
| Tap Strap 2 | `00002A00` / `C3FF0003` | `54 61 70 5F 44 34 32 35 32 36 31 31` | `Tap_D4252611` |

---

## Constraints and validation

Validation occurs in the **Tauri command layer** before the value is passed to `tap-ble`.
`TapDevice::set_name` trusts its input.

| Rule | Detail |
|------|--------|
| Minimum length | 1 character (empty names are not meaningful) |
| Maximum length | 20 characters — conservative limit that fits well within BLE ATT MTU and matches Tap's own naming scheme |
| Allowed characters | Printable ASCII only (`0x20`–`0x7E`); rejects non-ASCII to avoid encoding surprises across OS BLE stacks |
| Whitespace | Leading and trailing whitespace is trimmed before validation |

If validation fails, the Tauri command returns an `Err(String)` describing the rule violated.
The error is displayed inline in the UI without dismissing the rename input.

---

## tap-ble API

Add to `TapDevice` in `crates/tap-ble/src/tap_device.rs`:

```rust
/// Write a new friendly name to the device.
///
/// The name is written to both the Tap proprietary name characteristic
/// (`C3FF0003`) and the standard GAP Device Name characteristic (`00002A00`).
/// The change takes effect after the device reconnects.
///
/// The caller is responsible for validating the name before calling this
/// method (length, allowed characters).
pub async fn set_name(&self, name: &str) -> Result<(), BleError> { ... }
```

### Constants to add in `crates/tap-ble/src/tap_device.rs`

```rust
/// Tap proprietary device name copy (service C3FF0001)
const CHAR_TAP_DEVICE_NAME: Uuid = uuid!("c3ff0003-1d8b-40fd-a56f-c7bd5d0f3370");
```

`CHAR_TAP_DEVICE_NAME` is looked up with `peripheral.characteristics()` at the time of the
call (same pattern as existing characteristic lookups in `connect()`).

### Error variant

No new `BleError` variant is needed — `BleError::Btleplug` (via `#[from] btleplug::Error`)
covers a failed write.

---

## Tauri command

Add to `apps/desktop/src-tauri/src/commands.rs`:

```rust
/// Rename a connected Tap device.
///
/// `address` — the BDAddr string identifying the device (same format used
/// elsewhere in the app).
/// `name` — the desired new name (1–20 printable ASCII chars; leading/trailing
/// whitespace is trimmed automatically).
///
/// Returns `Ok(())` on success. Returns `Err(message)` if the device is not
/// connected, the name fails validation, or the BLE write fails.
#[tauri::command]
pub async fn rename_device(
    address: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> { ... }
```

Register in `invoke_handler` in `main.rs` / `lib.rs` alongside existing commands.

Add wrapper to `apps/desktop/src/lib/commands.ts`:

```ts
/**
 * Rename a connected Tap device.
 *
 * @param address - BDAddr string of the target device.
 * @param name - New friendly name (1–20 printable ASCII chars).
 * @throws {string} error message if the device is not connected, name is
 *   invalid, or the BLE write fails.
 */
export async function renameDevice(address: string, name: string): Promise<void> {
  return invoke('rename_device', { address, name });
}
```

---

## Frontend

The rename UI lives in the connected devices table (the same component that shows device address,
battery level, and role).

### Interaction design

1. Each device row has a small edit icon (pencil) next to the device name.
2. Clicking the icon replaces the name text with an inline `<input>` pre-filled with the current
   name, and shows Confirm (✓) and Cancel (✗) buttons.
3. Pressing Enter or clicking Confirm calls `renameDevice(address, trimmedName)`.
4. On success: update the displayed name optimistically; show a transient notice:
   *"Name saved — reconnect the device to see the new name in scan results."*
5. On error: show the error message inline below the input (do not clear the input).
6. Pressing Escape or clicking Cancel reverts to the static display without saving.

### Store changes

- `deviceStore` already holds a device record per address. Add an optional `friendlyName` field
  (`string | null`). The rename command updates this field on success.
- The friendly name is display-only in the store — it is not written to the persisted profile
  or `DeviceRegistry`. The BLE name on the device itself is the source of truth.

---

## Error handling

| Scenario | Error surface |
|----------|--------------|
| Device not found in app state (disconnected between UI action and command) | `Err("Device not connected")` from Tauri command |
| Name fails validation (empty, too long, non-ASCII) | `Err("…")` from Tauri command — message describes the violation |
| BLE write to `C3FF0003` fails | `Err("BLE driver error: …")` from Tauri command |

---

## Testing strategy

### Unit tests (`crates/tap-ble`)

- Test that `set_name` constructs the correct byte sequence (UTF-8, no framing) for a sample
  name — this can be tested without a live device by inspecting the bytes passed to the mock
  peripheral.
- If `btleplug` does not provide a mockable peripheral trait, document a manual verification
  step (same pattern used in Epic 9 mouse tests).

### Unit tests (Tauri command)

- `rename_device_empty_name_returns_error`
- `rename_device_too_long_returns_error` (21 chars)
- `rename_device_non_ascii_returns_error`
- `rename_device_whitespace_only_returns_error`
- `rename_device_valid_name_trims_whitespace` (confirm trimmed value is passed through)

### Manual hardware verification

1. Connect a TapXR or Tap Strap 2.
2. Enter a new name via the UI rename input (e.g. `MyTap`).
3. Confirm — no error shown.
4. Disconnect and re-pair the device.
5. Verify the new name appears in the device list on reconnect.
6. Repeat with a Tap Strap 2 if available.
