# tap-mapper — implementation plan

## Current focus

**Next task:** none — Epic 15 complete. Phase 2 (15.15–15.18 BluetoothHidDevice) is post-release only.
**Epic:** 15 — Android port
**Blocker:** none
**Pending decisions:** none

---

## How to use this document

Tasks are tracked with GitHub-style checkboxes:

- `- [ ]` — not started or in progress
- `- [x]` — completed

When a task is finished, change `[ ]` to `[x]`. Any AI assistant or contributor updating this
document should only mark a task complete when the implementation is merged and verified, not
when it is merely drafted or in review.

Tasks are grouped into epics. Each epic represents a coherent deliverable that can be tested
independently. Epics are roughly ordered by dependency — earlier epics unblock later ones — but
within an epic tasks can often be parallelised.

---

## Epics 0–8 — Complete

> Toolchain setup, data model, engine, BLE layer, Tauri commands, Svelte core UI, finger
> pattern widget, live visualiser / debug panel, and critical bug fixes. All tasks complete.
> See git log for full history.

Notable deviations from original spec:
- **0.6, 0.7** (CI pipeline, justfile) — deferred to Epic 14 (packaging)
- **1.18** (`OverloadStrategy` enum) — removed; patient behaviour is now automatic
- **2.11** (eager overload) — removed along with 1.18
- **8.2** (eager bug fix) — superseded by removal of eager strategy

---

## Epics 9–14, 16–18 — Complete

> Mouse actions, device renaming, context-aware profile switching, system tray, design system
> consistency, packaging and distribution, event notifications, extended keyboard key support,
> and haptic feedback. All tasks complete. See git log for full history.

Notable deviations / carry-overs:
- **14.5** macOS code signing — skipped (no Apple Developer account)
- **14.6** Windows code signing — pending external SignPath.io application; can ship without it
- Epics 16, 17, 18 were implemented before Epic 15 (Android) as they had no cross-platform dependency

---

## Epic 15 — Android port

> Goal: bring the full mapxr experience to Android. Phase 1 mirrors desktop functionality —
> Tap device connects to the phone and tap actions are injected into the foreground Android
> app via AccessibilityService. Phase 2 (post-release) adds BluetoothHidDevice relay so the
> phone acts as a keyboard adapter for a paired computer.
> Spec: `docs/spec/android-spec.md`
> Dependencies: Epics 1, 2 (mapping-core), 5, 6, 7 (frontend)

### 15a — Project setup and infrastructure

- [x] **15.1** _(spec §3)_ Run `cargo tauri android init`; configure `minSdkVersion 26`, `targetSdkVersion 34`; add the Android job to `release.yml` (signed APK uploaded to GitHub Releases)
- [x] **15.2** _(spec §8)_ Add Android path to `platform.rs`; add Android-specific fields to `Preferences`; add `get_platform` Tauri command
- [x] **15.3** _(spec §10)_ Write `AndroidManifest.xml` with all Phase 1 permissions; implement runtime permission request flow for `BLUETOOTH_SCAN` / `BLUETOOTH_CONNECT` from `BlePlugin`

### 15b — Kotlin BLE plugin

- [x] **15.4** _(spec §4.1–4.3)_ Write `BlePlugin.kt`: scan for Tap devices (`ble-device-found` events), connect via `BluetoothLeGatt`, perform GATT setup and enter controller mode; emit `ble-device-connected` / `ble-device-disconnected` events matching desktop signatures
- [x] **15.5** _(spec §4.4)_ Implement the JNI bridge: export `processTapBytes(address, bytes): String` from `mapping-core`; call it from `BlePlugin` on each characteristic notification; deserialise returned `Vec<Action>` JSON
- [x] **15.6** _(spec §4.5)_ Implement BLE reconnection policy (exponential backoff, 5 retries); handle Android-specific GATT quirks (address randomisation, GATT cache refresh, connection interval request)

### 15c — Foreground Service and battery

- [x] **15.7** _(spec §5)_ Write `MapxrForegroundService.kt`: persistent notification (`LOW` importance channel, content shows device count + active profile, "Stop" action); start/stop service from `BlePlugin` on device connect/disconnect; add `start_foreground_service` / `stop_foreground_service` Tauri commands
- [x] **15.8** _(spec §6)_ Write `BatterySetupWizard.svelte`: OEM detection via `get_oem_info` command, manufacturer-specific deep-link instructions for Xiaomi/Samsung/Huawei/Oppo; `request_battery_exemption` step; record completion in `preferences.json`

### 15d — AccessibilityService key injection

- [x] **15.9** _(spec §7.2–7.3)_ Write `MapxrAccessibilityService.kt` (minimal — `typeNone`, no content scanning); write `AccessibilityPlugin.kt` with `checkAccessibilityEnabled` / `openAccessibilitySettings` commands; wire `dispatchKeyEvent()` calls from BLE action dispatch path
- [x] **15.10** _(spec §7.4)_ Implement full key mapping table (`mapping-core Key` → `KeyEvent.KEYCODE_*`); handle unsupported keys (Insert, PrintScreen, etc.) as no-op with log warning; implement `TypeText` via `ACTION_MULTIPLE`; implement mouse click/scroll via `GestureDescription`
- [x] **15.11** _(spec §7.5)_ Write `AccessibilitySetupPrompt.svelte` and `AndroidOnboarding.svelte`; show setup modal on first device connection if accessibility not enabled; add accessibility + battery status sections to Settings page

### 15e — Integration and release

- [x] **15.12** _(spec §9)_ Hide desktop-only Settings sections on Android (tray, start-at-login); verify all shared commands (`start_scan`, `connect_device`, profile commands) work identically on Android without Svelte changes
- [x] **15.13** _(spec §13)_ Manual test run against device matrix: Pixel (stock Android 14), Samsung (One UI 6), Xiaomi (MIUI 14); document results in `docs/testing/android-manual-tests.md`
- [x] **15.14** _(spec §14)_ Publish signed APK to GitHub Releases; submit to F-Droid (manual process)

### 15f — Phase 2: BluetoothHidDevice relay *(implement only after 15.14 is released)*

- [ ] **15.15** _(spec §11.2–11.3)_ Write `HidPlugin.kt`: register phone as Bluetooth HID device; implement `hid_start_host`, `hid_stop_host`, `hid_list_paired_targets`, `hid_connect_target`, `hid_disconnect_target` commands; emit `hid-target-connected` / `hid-target-disconnected` events
- [ ] **15.16** _(spec §11.4)_ Implement HID report encoding: keyboard boot report (modifier + 6 keys), consumer control report (media keys), mouse report; write unit tests for all key categories and modifier combinations
- [ ] **15.17** _(spec §11.5–11.6)_ Add `android_output_mode` preference (`"direct"` | `"relay"`); write `HidTargetPairing.svelte`; add output mode selector to Settings; gate action dispatch on active mode
- [ ] **15.18** _(spec §11.7 + §13)_ Add `BLUETOOTH_ADVERTISE` permission; manual test Phase 2 against Windows 11 and macOS 14; publish updated APK

---

## Stretch goals (tracked but not scheduled)

> These are explicitly out of scope for the initial release. Listed here so they are not forgotten
> and so the schema/architecture decisions that accommodate them are visible.

- [ ] **S.1** Raw sensor / gesture triggers: tap-hold simulation via accelerometer duration, wrist rotation, air swipe. Requires subscribing to raw sensor mode notifications and building a gesture recognition pipeline on top of the 200Hz accelerometer stream.
- [ ] **S.2** iOS port: WKWebView host + Swift BLE bridge. Shares the same Svelte frontend. Lower priority than Android.
- [ ] **S.3** Community profile repository: a hosted index of shared profiles with an in-app browser and one-click import.
- [ ] **S.4** Plugin / scripting API: allow profiles to invoke a local HTTP endpoint or run a Lua script as an action, for integration with external tools (OBS, stream decks, home automation).
- [ ] **S.5** Profile normalisation CLI tool: `tap-mapper validate/normalize/migrate/lint` commands for working with profile files outside the GUI. Spec exists at `docs/spec/cli-tool-spec.md`.

---

## Dependency map

```
Epics 0–18 (complete, except 14.6 pending external)
  └── Epic 15 (Android) ← depends on Epics 1+2 (mapping-core) and Epics 5+6+7 (frontend)
  └── S.1–S.5 (stretch goals, unscheduled)
```
