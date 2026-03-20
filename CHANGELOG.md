# Changelog

All notable changes to mapxr are documented here.

## Versioning

mapxr follows [Semantic Versioning](https://semver.org/):

- **MAJOR** — breaking change to the profile JSON schema (existing profiles will not load without migration)
- **MINOR** — new feature; existing profiles continue to work unchanged
- **PATCH** — bug fix or internal improvement; no schema or behaviour change

Pre-release builds use the suffix convention `v1.2.3-beta.1`, `v1.2.3-rc.1`, etc.

---

## [Unreleased]

### Added
- Mouse click and scroll actions (`mouse_click`, `mouse_double_click`, `mouse_scroll`)
- Device renaming via BLE GATT characteristic
- Context-aware automatic profile switching (Linux and Windows)
- System tray with background operation and start-at-login support
- Design system shared between desktop app and marketing site
- Desktop notifications for device connect/disconnect and profile/layer changes
- Extended keyboard key support: F1–F24, media keys, and system keys
- Haptic feedback via Tap Strap vibration motor: explicit action type and event-driven triggers
- In-app update checking via `tauri-plugin-updater`

---

## [0.1.2] — 2026-03-20

### Fixed
- Tray icon crash on startup when installed via RPM or DEB — icon is now embedded in the binary rather than looked up at runtime, so it works regardless of install path
- Windows CI build error caused by a breaking API change in the `windows` crate 0.58 (`HWND.0` type and `GetWindowTextW` signature changed in the context-switching focus monitor)
- Auto-updater `latest.json` was missing from releases — added the required `createUpdaterArtifacts` bundle config so Tauri produces signed update artifacts and `tauri-action` publishes the manifest automatically

---

<!-- Add new releases above this line in the format below:

## [1.0.0] — YYYY-MM-DD

### Added
- …

### Changed
- …

### Fixed
- …

### Removed
- …

-->
