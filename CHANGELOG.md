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
- **Android port (Phase 1)** — run MapXr on Android 11+ (API 30+); connect a Tap Strap via Bluetooth and inject gestures as keystrokes into any app
  - BLE scan, connect, and reconnect via `BlePlugin` Kotlin plugin with runtime permission handling
  - Foreground service (`MapxrForegroundService`) with persistent notification showing device count, active profile, and keyboard status; `foregroundServiceType=connectedDevice`
  - **Shizuku key injection** — full action vocabulary dispatched via Shizuku's `InputManager.injectInputEvent()` running as shell uid (2000); no root required; supports: `key`, `key_chord`, `type_string`, `mouse_click`, `mouse_double_click`, `mouse_scroll`, `macro`, `vibrate`; complete key mapping table for all `VALID_KEYS`
  - Shizuku setup wizard (`ShizukuSetup`) — 3-step guided flow (Install → Start → Grant permission); polls state every 1 s and auto-advances; "✓ Active" confirmation screen
  - Background key injection via JNI (`NativeBridge` + `android_jni.rs`): tap bytes flow directly from `BlePlugin` → `ShizukuDispatcher.dispatch()` without going through the WebView, so injection works when the app is backgrounded or the screen is off
  - OEM battery restriction wizard (`BatterySetupWizard`) with manufacturer-specific deep-link instructions for Xiaomi, Samsung, Huawei/Honor, OPPO, OnePlus, Realme, Vivo
  - First-launch onboarding: sequences battery setup automatically on first run (`AndroidOnboarding`)
  - Android-specific Settings sections: Keyboard Mode (Shizuku status badge + setup wizard), background operation / battery exemption status, auto-start service toggle
  - Signed APK published to GitHub Releases on each version tag via `release.yml` `build-android` job

---

## [1.0.0] — 2026-03-20

Initial release.

### Added
- **Two-device support** — connect a left and right Tap Strap simultaneously for up to 1023 unique single-tap chords across 10 fingers
- **Profile editor** — create and manage tap mappings in-app without editing JSON
- **Layer stack** — push, pop, and switch layers programmatically; build modal and context-sensitive input systems
- **Full action vocabulary** — `key`, `key_chord`, `type_string`, `macro`, `push_layer`, `pop_layer`, `switch_layer`, `toggle_variable`, `set_variable`, `block`, `alias`, `mouse_click`, `mouse_double_click`, `mouse_scroll`, `vibrate`
- **Extended key support** — F1–F24, media keys (play/pause, next/prev, volume), and system keys (Insert, Print Screen, Scroll Lock, etc.)
- **Trigger types** — single tap, double tap, triple tap, and ordered sequence triggers
- **Variables** — named boolean/integer values in profiles; `toggle_variable` enables single-tap toggle actions (e.g. mute/unmute)
- **Device renaming** — assign friendly names to Tap devices via BLE
- **Context-aware profile switching** — automatically activate profiles based on the focused application (Linux and Windows)
- **System tray** — runs in the background with a tray icon; close-to-tray behaviour and start-at-login configurable in Settings
- **Desktop notifications** — OS notifications for device connect/disconnect and profile/layer changes; each event type independently toggleable
- **Haptic feedback** — vibration patterns on tap confirmation, layer switch, and profile switch; explicit `vibrate` action type for custom sequences
- **In-app updater** — checks for new releases automatically; dismissible banner and install-and-restart dialog
- **Debug panel** — live event stream with resolved/unmatched/combo-timeout events, timing bars, and export to JSONL

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
