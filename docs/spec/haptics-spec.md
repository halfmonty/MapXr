---
covers: Epic 18 (Haptic feedback)
status: Approved
last-updated: 2026-03-19
---

# Haptic feedback — specification (Epic 18)

## Table of contents

1. [Overview](#overview)
2. [GATT protocol](#gatt-protocol)
3. [VibrationPattern type](#vibrationpattern-type)
4. [vibrate action type](#vibrate-action-type)
5. [Event-triggered haptics](#event-triggered-haptics)
6. [Built-in patterns](#built-in-patterns)
7. [Settings schema](#settings-schema)
8. [Implementation task breakdown](#implementation-task-breakdown)

---

## Overview

Mapxr can send vibration commands to connected Tap Strap / TapXr devices over BLE. This enables
two use cases:

- **Explicit action**: a profile binding fires a `vibrate` action with a user-defined pattern.
- **Event-triggered**: the app automatically vibrates on tap receipt, layer switch, or profile
  switch — each individually toggleable in Settings.

No intensity control is available at the hardware level. Patterns are binary (motor on or off)
and expressed as alternating on/off durations.

---

## GATT protocol

### Characteristic

| Field | Value |
|-------|-------|
| UUID | `C3FF0009-1D8B-40FD-A56F-C7BD5D0F3370` |
| Service | Tap proprietary (`C3FF0001-1D8B-40FD-A56F-C7BD5D0F3370`) |
| Label in probe output | `"Haptic"` |
| Properties | `WwR W` (WriteWithoutResponse preferred; Write also supported) |
| Name in Python SDK | `ui_cmd_characteristic` |

Sources:
- `docs/reference/gatt-characteristics.txt` — live probe of a TapXR FW 3.4.0 device; line
  `C3FF0009  Haptic  | WwR W  | Trigger haptic feedback pattern`
- `docs/reference/vibration.txt` — Python SDK implementation showing payload format and
  encoding algorithm (`write_gatt_char(TapUUID.ui_cmd_characteristic, …)`)

### Payload format

```
[0x00, 0x02, d0, d1, d2, ..., dN]
```

- Byte 0: `0x00` — reserved / sub-command prefix
- Byte 1: `0x02` — vibration sub-command
- Bytes 2…N: encoded duration sequence (N ≤ 18)

### Duration encoding

Each raw duration `t` (milliseconds) is encoded to a single byte `b`:

```
b = clamp(t / 10, 0, 255)
```

- Valid raw range: 10–2550 ms (maps to `0x01`–`0xFF`)
- Resolution: 10 ms
- A value of `0` means 0 ms, which has no effect for either on or off phases

### Sequence semantics

- Elements alternate **on, off, on, off, …** starting with **on** at index 0.
- Maximum 18 elements per write (9 on+off pairs).
- Sequences longer than 18 elements are truncated at 18 before encoding.
- An empty sequence is a no-op; the write is skipped.

### Example

```python
# Python SDK example from docs/reference/vibration.txt
tap_device.send_vibration_sequence([1000, 300, 200])
# → 1 s on, 300 ms off, 200 ms on
# Encoded payload: [0x00, 0x02, 100, 30, 20]
```

---

## VibrationPattern type

### Rust

```rust
/// A haptic vibration pattern: alternating on/off durations in milliseconds.
///
/// - Elements alternate on, off, on, off, … starting with on at index 0.
/// - Each duration must be in [10, 2550] ms with 10 ms resolution.
/// - Maximum 18 elements (9 on/off pairs); longer sequences are truncated before sending.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VibrationPattern(pub Vec<u16>);
```

Validation rules (enforced in `VibrationPattern::validate()` and on construction):

| Rule | Detail |
|------|--------|
| Each element ≥ 10 ms | Values below 10 ms are clamped to 10 ms during encoding |
| Each element ≤ 2550 ms | Values above 2550 ms are clamped to 2550 ms during encoding |
| Resolution 10 ms | Values are rounded down to the nearest 10 ms during encoding |
| Max 18 elements | Sequences longer than 18 are truncated at the BLE send site, not rejected |
| Non-empty | A zero-length pattern is a no-op (no write issued); it is not an error |

Clamping and truncation happen silently at the BLE send site, not during profile validation.
Profile validation does not reject out-of-range values — it is the send path's responsibility
to clamp them, ensuring no data loss during profile export/import.

### JSON schema

```json
{
  "type": "array",
  "items": { "type": "integer", "minimum": 10, "maximum": 2550 },
  "maxItems": 18
}
```

---

## vibrate action type

Adds a new `Action::Vibrate` variant to `mapping-core`.

### JSON representation

```json
{ "action": "vibrate", "pattern": [200, 100, 200, 100, 200] }
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `action` | `"vibrate"` | Yes | Discriminant |
| `pattern` | `VibrationPattern` | Yes | Alternating on/off durations in ms |

### Dispatch behaviour

- The vibrate action is dispatched to **all currently connected devices** by default.
- There is no per-device targeting — Tap devices are worn on different hands and vibrating all
  of them simultaneously is always the expected behaviour.
- If no device is connected the action is silently dropped.
- The BLE write uses WriteWithoutResponse (`WwR`) for latency; the characteristic also supports
  Write-with-response (`W`) as confirmed by the probe output, but the Python SDK uses
  without-response and that is what we follow.

### Mapping-core changes

1. Add `Action::Vibrate { pattern: VibrationPattern }` to the `Action` enum.
2. No engine-level semantics change — `Vibrate` is a leaf action like `Key` and `MouseClick`.
3. Update `engine_output.rs` dispatch table to include `Vibrate`.

---

## Event-triggered haptics

Automatic vibration on system events, gated on `haptics_enabled` and per-event toggles.

| Event | Default pattern | Toggle key |
|-------|----------------|------------|
| Tap received (any) | Short pulse: `[80]` | `haptic_on_tap` |
| Layer switch | Double pulse: `[80, 80, 80]` | `haptic_on_layer_switch` |
| Profile switch | Triple pulse: `[80, 80, 80, 80, 80]` | `haptic_on_profile_switch` |

Notes:
- "Tap received" fires for every resolved tap event (after combo resolution, not on raw input).
  This provides immediate confirmation that the device registered the gesture.
- Event-triggered patterns are not user-configurable in the initial implementation (they use the
  built-in patterns above). Per-event pattern customisation is a stretch goal.
- All event-triggered haptics respect `haptics_enabled`; if that is false, no vibration fires
  regardless of the per-event toggles.

---

## Built-in patterns

Named constants defined in `tap-ble` for use by the event-trigger dispatch code:

| Constant | Pattern (ms) | Description |
|----------|-------------|-------------|
| `PATTERN_SHORT_PULSE` | `[80]` | Single brief buzz (tap confirmation) |
| `PATTERN_DOUBLE_PULSE` | `[80, 80, 80]` | Two pulses (layer switch) |
| `PATTERN_TRIPLE_PULSE` | `[80, 80, 80, 80, 80]` | Three pulses (profile switch) |

---

## Settings schema

Extends `Preferences` / `StoredPreferences` in `src-tauri/src/state.rs`.

New fields:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `haptics_enabled` | `bool` | `true` | Master toggle; gates all vibration |
| `haptic_on_tap` | `bool` | `false` | Vibrate on every resolved tap event |
| `haptic_on_layer_switch` | `bool` | `true` | Vibrate on layer push/pop/switch |
| `haptic_on_profile_switch` | `bool` | `true` | Vibrate on profile activate |

`haptic_on_tap` defaults to `false` because it fires very frequently (every tap) and may
become distracting. Users can enable it explicitly.

### Tauri DTO extension

`TrayPreferences` in `commands.rs` gains these four fields (same names, same types).
`get_preferences` and `save_preferences` are updated accordingly.

### Settings page placement

A new "Haptics" section is added to the existing Settings page (`/settings`), below the
"Notifications" section:

```
Haptics
  [toggle] Enable haptic feedback          (haptics_enabled)

  When enabled:
  [toggle] Vibrate on tap                  (haptic_on_tap)
  [toggle] Vibrate on layer switch         (haptic_on_layer_switch)
  [toggle] Vibrate on profile switch       (haptic_on_profile_switch)
```

The three per-event toggles are greyed out when `haptics_enabled` is false.

---

## Manual hardware verification

Because `TapDevice::vibrate()` writes to real BLE hardware, automated unit tests cannot
cover the end-to-end path. The following manual steps verify correct behaviour against a
physical device.

### Prerequisites

- A Tap Strap or TapXR device paired and connected via the app.
- The Settings page is accessible (`/settings`).

### Steps

1. **Basic vibrate action**
   - In the profile editor, assign a tap code to a `vibrate` action with pattern `[200, 100, 200]`.
   - Fire the tap code on the physical device.
   - Expected: three distinct pulses (200 ms on, 100 ms off, 200 ms on) felt in the wrist strap.

2. **Haptic on tap**
   - In Settings → Haptics, enable "Vibrate on tap".
   - Fire any tap on the physical device.
   - Expected: a single short buzz (~80 ms) immediately after each tap is resolved.

3. **Haptic on layer switch**
   - Configure a binding that pushes or pops a layer.
   - Fire the binding.
   - Expected: two short pulses (80 ms on, 80 ms off, 80 ms on).

4. **Haptic on profile switch**
   - Use `activate_profile` to switch to a different profile (via the Devices page or a binding).
   - Expected: three short pulses (on, off, on, off, on pattern with 80 ms each).

5. **Master toggle disables all vibration**
   - Disable "Enable haptic feedback" in Settings.
   - Fire a tap, switch a layer, and execute a `vibrate` action.
   - Expected: no vibration for any of these.

6. **Empty pattern is silent**
   - Assign a `vibrate` action with an empty pattern `[]`.
   - Fire it.
   - Expected: no vibration (no BLE write is issued; no crash or error).

7. **18-element truncation**
   - Assign a `vibrate` action with 20 alternating durations (e.g. `[80, 80, …]` × 20).
   - Fire it.
   - Expected: device vibrates for 18 phases only; no crash or error.

8. **No device connected**
   - Disconnect the Tap device, then fire a `vibrate` action from the profile.
   - Expected: no vibration, no error surfaced to the user (silent drop).

---

## Implementation task breakdown

| Task | Description |
|------|-------------|
| **18.1** | _(this spec)_ Research and document |
| **18.2** | Implement `VibrationPattern`, `TapDevice::vibrate()` in `tap-ble`; add `UI_CMD_UUID` constant; write unit tests for encoding |
| **18.3** | Add `Action::Vibrate { pattern }` to `mapping-core`; update dispatch in `src-tauri`; write serde round-trip tests |
| **18.4** | Svelte action editor: vibrate pattern builder (add/remove segments, duration inputs) |
| **18.5** | Extend `Preferences` with `haptics_enabled` + per-event flags; wire global gate |
| **18.6** | Event-driven haptics: tap received, layer switch, profile switch — wired in `pump.rs` |
| **18.7** | Settings UI: Haptics section with master toggle + per-event toggles |
| **18.8** | Unit tests for `VibrationPattern` serde; manual hardware verification steps documented |
