# System Tray and Background Operation — Spec

**Epic:** 12
**Status:** Draft — awaiting approval
**Dependencies:** Epics 4, 5 (Tauri commands, Svelte UI)

---

## Overview

The app should be able to run as a background service — visible in the system tray but with its
main window hidden. This is the normal mode of operation after first setup: the user configures
their profiles and then closes the window without quitting; BLE connections and tap-to-key
mapping remain active.

---

## 1. Tray icon and menu

### 1.1 Icon

- A single icon image is used in the system tray (format: `.ico` for Windows, `.png` for Linux/macOS).
- The icon is always the same (no active/inactive variant for MVP — can be added later).
- The icon is placed in `apps/desktop/src-tauri/icons/` alongside the existing app icons.

### 1.2 Context menu

Right-clicking the tray icon (or left-clicking on platforms that show a menu) shows:

| Menu item | Behaviour |
|---|---|
| **Show** | Brings the main window to the front (or un-hides it). Greyed out when the window is already visible and focused. |
| **Hide** | Hides the main window. Greyed out when the window is already hidden. |
| _(separator)_ | — |
| **Active profile: `<name>`** | Greyed-out informational label showing the currently active profile name. Shows "None" when no profile is active. |
| _(separator)_ | — |
| **Quit** | Exits the application fully (disconnects BLE, destroys the window). |

Left-clicking the tray icon toggles Show/Hide (show if hidden, hide if visible).

The "Active profile" label updates dynamically whenever the active profile changes.

---

## 2. Window close behaviour

### 2.1 Default: minimise to tray

By default, pressing the window close button **hides** the window instead of quitting.
The app continues running in the background with BLE active.

This is controlled by a `close_to_tray` preference (see §4).

### 2.2 First-hide notification

The first time the window is hidden to tray (i.e. the first time the user closes the window),
show a native OS notification: _"tap-mapper is still running in the background. Click the tray
icon to bring it back."_

Track this with a `shown_tray_hint` boolean in preferences so the notification fires only once.

### 2.3 Override: exit on close

If `close_to_tray = false`, the close button quits the application normally (same as Quit in the
tray menu). In this case the tray icon is still present while the app is running.

---

## 3. Tray tooltip

The tray icon tooltip shows a two-line string:

```
tap-mapper
<profile_name> · <N> device(s) connected
```

Examples:
- `"tap-mapper\nGaming · 2 devices connected"`
- `"tap-mapper\nNo profile active · 0 devices connected"`

The tooltip updates dynamically on profile change and device connect/disconnect.

---

## 4. Settings schema

New fields added to `preferences.json` (bumps `version` to `2`; missing fields default as shown):

```json
{
  "version": 2,
  "profile_active": true,
  "last_active_profile_id": "my-profile",
  "close_to_tray": true,
  "start_minimised": false,
  "start_at_login": false,
  "shown_tray_hint": false
}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `close_to_tray` | `bool` | `true` | Hide window on close instead of quitting |
| `start_minimised` | `bool` | `false` | Launch directly to tray without showing the window |
| `start_at_login` | `bool` | `false` | Register the app to start automatically at OS login |
| `shown_tray_hint` | `bool` | `false` | Whether the first-hide notification has been shown |

The existing `version: 1` format remains valid; missing new fields are filled in with defaults.

---

## 5. Settings page (Svelte)

A new route `apps/desktop/src/routes/settings/+page.svelte` is added. Nav label: **Settings**.

### 5.1 Settings sections

**Window behaviour**

- `close_to_tray` — toggle: "Minimise to tray when closed" (default: on)
- `start_minimised` — toggle: "Start minimised (launch directly to tray)" (default: off)

**System**

- `start_at_login` — toggle: "Start tap-mapper at system login" (default: off)
  - Disabled and labelled "(not supported on this platform)" on unsupported platforms.

### 5.2 Save behaviour

Each toggle saves immediately on change (same pattern as context-rules page). No explicit Save
button — changes are live. A subtle "Saved" flash confirmation is shown after each write.

### 5.3 Tauri commands

| Command | Signature | Description |
|---|---|---|
| `get_preferences` | `() -> TrayPreferences` | Return current tray-related preference fields |
| `save_preferences` | `(prefs: TrayPreferences) -> Result<(), String>` | Persist and apply changes (e.g. register/deregister login item) |

`TrayPreferences` TypeScript interface:
```typescript
interface TrayPreferences {
  close_to_tray: boolean;
  start_minimised: boolean;
  start_at_login: boolean;
}
```

---

## 6. Start at login — per-platform implementation

| Platform | Mechanism |
|---|---|
| **Windows** | Write/delete `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\tap-mapper` |
| **macOS** | Write/delete `~/Library/LaunchAgents/com.mapxr.tap-mapper.plist` |
| **Linux** | Write/delete `~/.config/autostart/tap-mapper.desktop` |

### 6.1 Linux `.desktop` file

```ini
[Desktop Entry]
Type=Application
Name=tap-mapper
Exec=/path/to/tap-mapper
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
```

The `Exec` path is obtained at runtime from `std::env::current_exe()`.

### 6.2 macOS LaunchAgent plist

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key><string>com.mapxr.tap-mapper</string>
    <key>ProgramArguments</key>
    <array><string>/path/to/tap-mapper</string></array>
    <key>RunAtLoad</key><true/>
  </dict>
</plist>
```

### 6.3 Windows registry

Use `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run` via the `winreg` crate
(already in the dependency discussion for other epics; add it here if not already present).
Only needed on Windows; guard with `#[cfg(target_os = "windows")]`.

---

## 7. Dynamic updates (no restart required)

All settings take effect immediately without restarting:

- `close_to_tray` — the window close handler checks the current in-memory value at the time of
  the close event; no restart needed.
- `start_minimised` — only meaningful at launch; changing it at runtime has no visible effect
  but is saved for next launch.
- `start_at_login` — the login item is registered or deregistered immediately when the toggle
  changes.

---

## 8. Plugin / crate dependencies

| Need | Candidate | Notes |
|---|---|---|
| System tray | `tauri-plugin-tray` | Official Tauri 2.x plugin; already planned in task 12.2 |
| Notifications | `tauri-plugin-notification` | Official Tauri 2.x plugin; used for the first-hide hint (§2.2) |
| Windows registry | `winreg` 0.52 | Well-maintained; Windows-only; needed for start-at-login on Windows |

All three require user approval per dependency policy before being added to `Cargo.toml`.

---

## 9. Out of scope (MVP)

- Animated or status-variant tray icons (active vs inactive tap mapping state)
- Tray icon badge / overlay (device count, etc.)
- Global keyboard shortcut to toggle the window
- macOS menu bar extras (full native menu bar item instead of tray)
