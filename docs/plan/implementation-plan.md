# tap-mapper — implementation plan

## Current focus

**Next task:** 21.7 — manual test matrix
**Epic:** 21 — Android Shizuku key injection (replaces Epic 20)
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
- [x] **15.5** _(spec §4.4)_ ~~Implement the JNI bridge~~ **DEVIATION:** WebView event path used instead of JNI. `BlePlugin.onTapBytes()` calls `trigger("tap-bytes-received")` → JS → `invoke("process_tap_event")` → Rust pump. Actions dispatched via `tap-actions-fired` → JS → `invoke("dispatchActions")`. Works in foreground only; backgrounded WebView JS suspends the pipeline. Fix tracked in Epic 19.
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

## Epic 19 — Android background dispatch (JNI native path)

> Goal: tap actions are dispatched into the foreground app even when MapXr is in the background.
> The current pipeline routes everything through WebView JS, which Android suspends when
> backgrounded. This epic implements the JNI bridge originally specified in
> `docs/spec/android-spec.md §4.4` but not built in task 15.5.
>
> Architecture: BLE bytes arrive in Kotlin → JNI call feeds existing Rust pump via
> `blocking_send` (bypasses WebView for input) → pump processes event and resolves actions →
> calls registered Kotlin dispatch callback directly (bypasses WebView for output) →
> `AccessibilityService` injects the key/gesture. The WebView path is retained but
> repurposed: UI-only events (debug panel, finger visualiser, layer change notifications).
>
> Spec: `docs/spec/android-background-dispatch-spec.md`
> Dependencies: Epic 15 (Android port)

- [x] **19.1** _(spec §5.2, §9)_ Add `jni = "0.22"` to `apps/desktop/src-tauri/Cargo.toml`
  under `[target.'cfg(target_os = "android")'.dependencies]`. In `lib.rs` Android startup,
  after `Arc::new(app_state)`, store a clone of the `Arc<AppState>` and the `AppHandle` in
  module-level statics in a new `android_jni.rs` module so JNI functions can reach engine
  state without going through the Tauri command context.

- [x] **19.2** _(spec §5.1, §5.3)_ Create `apps/desktop/src-tauri/src/android_jni.rs`.
  Implement two JNI-exported functions (both `#[cfg(target_os = "android")]`):
  - `Java_com_mapxr_app_NativeBridge_processTapBytes(env, class, address: JString, bytes: JByteArray)`:
    extracts `address` and `bytes`, calls `tap_event_tx.blocking_send(TapEventMsg { address, bytes })`
    on the stored `AppState`. Returns `"ok"` string (actions come back via the pump output callback).
    Returns `"err"` on channel-closed or no state; logs reason.
  - `Java_com_mapxr_app_NativeBridge_registerDispatchCallback(env, class)`:
    stores the `JavaVM` (via `env.get_java_vm()`) and a `GlobalRef` to the `AccessibilityDispatcher`
    class in module-level statics so the Rust pump can call back into Kotlin without WebView.

- [x] **19.3** _(spec §7.2, §8)_ Modify `android_pump.rs` `process_android_outputs`: after
  serialising dispatchable `Vec<Action>` to JSON and emitting `tap-actions-fired` (kept for
  debug panel), also invoke the registered Kotlin callback from task 19.2 by attaching the
  stored `JavaVM` to the current thread and calling
  `AccessibilityDispatcher.dispatch(actionsJson)` as a JNI static method call. If no callback
  is registered (accessibility not set up), log a warning and continue.

- [x] **19.4** _(spec §6, §7.1)_ Kotlin changes:
  - Create `NativeBridge.kt`: `object NativeBridge` with `external fun processTapBytes(...)` and
    `external fun registerDispatchCallback()`. Call `registerDispatchCallback()` from
    `MainActivity.onCreate` after all plugins are registered.
  - Create `AccessibilityDispatcher.kt`: extract the `when (type) { "key" -> ... }` dispatch logic,
    `keyNameToCode`, and `modifiersToMetaState` from `AccessibilityPlugin` into a standalone
    `object AccessibilityDispatcher` with a `fun dispatch(actionsJson: String)` entry point.
  - Modify `BlePlugin.onTapBytes()`: call `NativeBridge.processTapBytes(address, bytes)` first
    (the native path); then retain the existing `trigger("tap-bytes-received", ...)` call for
    the sidebar finger visualiser and debug panel.
  - Slim `AccessibilityPlugin.dispatchActions()` to a thin wrapper that delegates to
    `AccessibilityDispatcher.dispatch(actionsArray.toString())`.

- [x] **19.5** _(spec §8, §10)_ Update `apps/desktop/src/lib/android-bridge.ts`: remove the
  `invoke("dispatchActions", ...)` call from the `tap-actions-fired` listener. The listener
  should only forward the event payload to the debug store. Verify the `tap-bytes-received`
  listener only updates the debug store (finger visualiser) and does not invoke any dispatch
  command.

- [ ] **19.6** _(spec §11)_ Manual test on device:
  - Background test: connect device → open text app → minimise MapXr → tap → key appears in
    foreground app (was broken before this epic).
  - Foreground test: MapXr visible → tap → key injected exactly once (no double-dispatch) →
    debug panel shows the resolved event.
  - Run `cargo clippy -- -D warnings` and `cargo test --workspace` with no failures.

---

## Epic 20 — Android shell server *(SUPERSEDED — do not implement)*

> **Superseded by Epic 21.** All tasks 20.1–20.8 were implemented but 20.9 (manual test)
> was never completed because the direct ADB approach was abandoned after manual testing
> revealed that Samsung/Android 16 (API 36) adbd rejects our pairing keys with
> "Invalid base64 key" despite the keys being structurally correct (524 bytes, valid base64,
> correct RSAPublicKey layout, matching the PC key format exactly). Root cause could not be
> determined without root access to the device. See `docs/adb-issues.md` for full findings.
>
> The entire Epic 20 codebase (AdbKey, AdbPairing, AdbConnection, ShellServerManager,
> ShellClientManager, ShellServerPlugin, ShellInputEvent, NativeBridge, shell-server DEX,
> android_jni.rs dispatch path, ShellServerSetup.svelte) is to be deleted as part of
> Epic 21.
>
> Spec: `docs/spec/android-shell-server-spec.md` (retained for historical reference)
> Dependencies: Epic 19 (JNI pump path)

### 20a — Shell server component

- [x] **20.1** _(spec §4.1)_ Create `shell-server/` Gradle module. Write `MapxrShellServer.kt`:
  binds abstract Unix socket `@mapxr_input`, reads length-prefixed `ShellKeyEvent` messages,
  calls `InputManager.injectInputEvent()` via reflection, emits 5-second heartbeat. Gradle task
  compiles to standalone DEX bundled as `app/src/main/assets/mapxr-shell-server.dex`.

### 20b — ADB client

- [x] **20.2** _(spec §4.2, §5.2)_ Evaluate dadb for Android-native compatibility. Add as
  dependency or adapt LADB source. Implement `AdbConnection.kt`: TLS 1.3 connect to Wireless
  Debugging port using stored RSA keypair, ADB CNXN handshake, open shell channel.

- [x] **20.3** _(spec §5.1)_ Implement SPAKE2+ pairing in `AdbPairing.kt` using Bouncy Castle:
  connect to pairing port, complete SPAKE2+ with user-supplied 6-digit code, exchange PeerInfo,
  store RSA keypair in Android Keystore. Unit tests with mock TLS server.

### 20c — Lifecycle and IPC

- [x] **20.4** _(spec §4.4)_ Write `ShellServerManager.kt`: startup orchestration (detect →
  connect ADB → deploy DEX → start server → wait for socket). Post-reboot auto-reconnect.
  `serverState: StateFlow<ServerState>` for UI.

- [x] **20.5** _(spec §4.3, §5.3)_ Write `ShellClientManager.kt`: Unix socket client,
  `send(ShellInputEvent)` with 16-event queue, heartbeat monitoring, async reconnect.

### 20d — Rust pump integration and rollback

- [x] **20.6** _(spec §4.5)_ Add `dispatchKeyEvent` JNI entry point in `android_jni.rs` →
  `ShellClientManager.sendActions()`. Replace `dispatch_via_jni` in `android_pump.rs` with
  `dispatch_via_shell` routing all actions to the shell server.

- [x] **20.7** _(spec §6)_ Execute full accessibility rollback: delete
  `AccessibilityDispatcher.kt`, `MapxrAccessibilityService.kt`, `AccessibilityPlugin.kt`,
  `AccessibilitySetupPrompt.svelte`, `accessibility_service_config.xml`; update
  `AndroidManifest.xml` (remove service + permission), `MainActivity.kt`, `NativeBridge.kt`,
  `android_jni.rs`, `android_pump.rs`; remove `dispatchActions` Tauri command if present.
  Verify all §6.3 grep checks pass.

### 20e — Setup UI

- [x] **20.8** _(spec §4.6)_ Write `ShellServerSetup.svelte` with 4-step pairing wizard.
  Add "Keyboard Mode" section to Settings (replacing accessibility section). Add status line
  to foreground service notification.

### 20f — Testing

- [ ] ~~**20.9**~~ *(cancelled — epic superseded by Epic 21)*

---

## Epic 21 — Android Shizuku key injection

> Goal: replace the abandoned Epic 20 (direct ADB shell server) with Shizuku-based key
> injection. Shizuku grants the app shell-uid access to `InputManager.injectInputEvent()`
> via a Binder IPC mechanism, achieving identical keyboard/mouse injection capability
> without requiring a reimplementation of the ADB wireless debugging protocol.
>
> User-facing setup: install Shizuku from Play Store / F-Droid, start it once via
> Wireless Debugging (same pairing screen the user already knows), grant mapxr permission.
> No further setup required across reboots (Shizuku auto-starts via Wireless Debugging).
>
> This epic also removes all Epic 20 code (ADB client, shell server, related UI).
>
> **Spec: `docs/spec/android-shizuku-spec.md` — DRAFT REQUIRED before any code**
> Dependencies: Epic 19 (JNI pump path)

- [x] **21.1** Draft `docs/spec/android-shizuku-spec.md`. *(Approved 2026-03-23)*

- [x] **21.2** _(spec §4, §3)_ Delete all Epic 20 files (AdbKey, AdbPairing, AdbConnection,
  ShellServerManager, ShellClientManager, ShellServerPlugin, ShellInputEvent, shell-server module,
  ShellServerSetup.svelte). Update `build.gradle.kts` — remove spake2/BouncyCastle/coroutines
  deps and the `preBuild` hook; add Shizuku `api:13.1.5` and `provider:13.1.5`. Update
  `build.gradle.kts` (root) — remove jitpack (no longer needed). Update `settings.gradle` —
  remove `:shell-server` include. Create `IInputService.aidl` (spec §7.1). Create
  `InputUserService.kt` (spec §7.2).

- [x] **21.3** _(spec §7.3)_ Write `ShizukuDispatcher.kt`: `ShizukuState` sealed class,
  `StateFlow<ShizukuState>`, `init(context)`, `requestPermission()`, `bind()` / `unbind()`,
  and `dispatch(actionsJson)` with full action-JSON → InputEvent conversion table
  (re-use key mapping from former AccessibilityDispatcher).

- [x] **21.4** _(spec §7.4, §7.5, §6)_ Write `ShizukuPlugin.kt` (`getShizukuState`,
  `requestShizukuPermission`, `openShizukuApp`). Update `NativeBridge.kt` —  remove
  `initDispatch` / `dispatchActions`; add `registerShizukuDispatcher()` external fun.
  Update `MainActivity.kt` — swap `ShellServerPlugin` → `ShizukuPlugin`; call
  `ShizukuDispatcher.init()`; call `NativeBridge.registerShizukuDispatcher()`. Update
  `MapxrForegroundService.kt` — read `ShizukuDispatcher.state` instead of `ShellServerManager`.
  Update `AndroidManifest.xml` — remove MANAGE_USB; add `ShizukuProvider` (spec §6.1).

- [x] **21.5** _(spec §8)_ Update `android_jni.rs` — rename `initDispatch` JNI function to
  `registerShizukuDispatcher`; rename stored class ref from `NativeBridge` to `ShizukuDispatcher`;
  update module-level statics and doc comments. Update `android_pump.rs` — rename
  `dispatch_via_shell` to `dispatch_via_shizuku`; update JNI method call to
  `ShizukuDispatcher.dispatch()`; update all comments.

- [x] **21.6** _(spec §9)_ Update `commands.ts` — remove shell server commands/types; add
  `ShizukuState` type + `getShizukuState`, `requestShizukuPermission`, `openShizukuApp`.
  Create `ShizukuSetup.svelte` (3-step wizard, spec §9.2). Update `settings/+page.svelte` —
  swap `ShellServerSetup` → `ShizukuSetup`; swap `getShellServerState` → `getShizukuState`.
  Verify cleanup grep (spec §4) returns no results.

- [ ] **21.7** _(spec §12)_ Manual test matrix: fresh install wizard, key injection, KeyChord,
  global shortcuts, TypeString, mouse click/scroll, background injection, reboot persistence,
  graceful degradation. `cargo clippy -- -D warnings` clean. `cargo test --workspace` passes.

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
  └── Epic 15 (Android port, Phase 1 complete; 15.15–15.18 post-release)
      └── Epic 19 (Android background dispatch — JNI fix for task 15.5 deviation)
          └── Epic 20 (Android shell server — SUPERSEDED, do not implement)
          └── Epic 21 (Android Shizuku key injection — replaces Epic 20)
  └── S.1–S.5 (stretch goals, unscheduled)
```
