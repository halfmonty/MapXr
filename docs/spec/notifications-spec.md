# Notifications spec — Epic 16

## Overview

Epic 16 surfaces OS desktop notifications for key app events. `tauri-plugin-notification` is
already a dependency (added in Epic 12). Each notification type is individually toggleable in
Settings so users can control which events produce noise.

---

## Notification events

| Event ID | When fired | Default enabled |
|----------|-----------|-----------------|
| `device_connected` | A Tap device successfully connects and is ready | `true` |
| `device_disconnected` | A connected Tap device disconnects | `true` |
| `layer_switch` | The active layer changes within the current profile | `false` |
| `profile_switch` | The active profile changes | `true` |

`layer_switch` defaults to `false` because users who switch layers frequently (e.g. via combos)
would find it very noisy.

---

## Notification payload

Each notification is informational only — no action buttons.

| Field | Type | Notes |
|-------|------|-------|
| `title` | string | Short, e.g. `"Device Connected"` |
| `body` | string | Contextual detail, e.g. `"TapStrap (Left) is ready"` |

### Body text templates

- **device_connected**: `"{device_name} is ready"`
- **device_disconnected**: `"{device_name} disconnected"`
- **layer_switch**: `"Switched to {layer_name}"`
- **profile_switch**: `"Switched to {profile_name}"`

`device_name` uses the user-assigned friendly name if set (Epic 10), otherwise the BLE
peripheral name. Layer and profile names come from the active profile document.

---

## Preferences schema extension

The following fields are added to the existing `Preferences` struct and persisted in
`preferences.json` alongside existing fields:

```json
{
  "notify_device_connected": true,
  "notify_device_disconnected": true,
  "notify_layer_switch": false,
  "notify_profile_switch": true
}
```

All four fields are optional in the JSON file; absent fields fall back to the defaults above.

---

## Settings UI placement

A new **"Notifications"** subsection is added to the existing Settings page, below the tray /
close-behaviour section. It contains four toggle rows:

| Toggle label | Description shown to user |
|--------------|--------------------------|
| Device connected | Notify when a Tap device connects |
| Device disconnected | Notify when a Tap device disconnects |
| Layer switched | Notify when the active layer changes |
| Profile switched | Notify when the active profile changes |

Changes to toggles take effect immediately (no restart required).

---

## Implementation rules

- All notification dispatch is gated: check `preferences.notify_<event>` before calling
  `tauri_plugin_notification`. Do not emit if the flag is `false`.
- Notifications are best-effort. Errors from the OS notification subsystem are logged at
  `warn` level but are **not** surfaced to the user and do not propagate as Tauri errors.
- No notification grouping or coalescing is required in v1.
- Notification dispatch happens on the Rust side (in `src-tauri`) where the events originate;
  the frontend does not emit notifications itself.
