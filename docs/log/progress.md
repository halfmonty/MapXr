## 2026-03-24 — 21.7: manual test complete; Epic 21 done

**Tasks completed:** 21.7
**Tasks in progress:** none

**Files changed:**

- `apps/desktop/src-tauri/src/lib.rs` — **root fix:** `"shellServer"` → `"shizuku"` in
  `run_mobile()` Kotlin plugin stub registration; without this Tauri returned "plugin shizuku
  not found" for every `getShizukuState` invoke
- `apps/desktop/src/routes/settings/+page.svelte` — initial `shizukuState` changed from
  `"Unsupported"` to `"NotRunning"` (prevents false red badge on load); `refreshAndroidStatus()`
  split so Shizuku fetch is independent of battery/prefs (a throw in either no longer blocks
  the other); background 2 s state-polling timer added so badge auto-updates after launch
- `apps/desktop/src-tauri/gen/android/app/src/main/AndroidManifest.xml` — added `<queries>`
  block for `moe.shizuku.privileged.api` (Android 11+ package-visibility; required for
  `getLaunchIntentForPackage` and `getPackageInfo` to see Shizuku)
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/ShizukuDispatcher.kt` —
  added `startStatePoller()`: polls `pingBinder()` every 2 s while state is
  `NotRunning`/`NotInstalled`, ensuring state advances even if `addBinderReceivedListenerSticky`
  fires before the provider is ready; `binderDeadListener` now also calls `startStatePoller()`
  on reconnect; added diagnostic `Log.d` calls to `updateState()` and `isShizukuInstalled()`;
  `startStatePoller` logs start/stop
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/ShizukuPlugin.kt` —
  added `android.util.Log` import; `openShizukuApp` wrapped in try-catch and primary launch
  intent now gets `FLAG_ACTIVITY_NEW_TASK`

**Notes:**
- Manual test on Samsung Android 16 device using sideloaded Shizuku APK (Play Store version
  does not support Android 16). Shizuku detects correctly, wizard progresses, key injection
  works.
- The single most impactful bug was `lib.rs` still registering `"shellServer"` as the Kotlin
  plugin stub. Tauri's invoke routing looks up the Rust stub first; without `"shizuku"` in the
  Rust registry every `plugin:shizuku|...` call failed before reaching Kotlin.
- `cargo clippy -- -D warnings` clean; `cargo test --workspace` passes.
- All planned epics (0–21) are now complete. Next work is unscheduled stretch goals or a
  release bump.

**Next:** no planned tasks remaining; stretch goals S.1–S.5 are unscheduled

---

## 2026-03-23 — 21.3–21.6: Shizuku dispatcher, plugin, JNI, and frontend

**Tasks completed:** 21.3, 21.4, 21.5, 21.6
**Tasks in progress:** none

**Files changed:**

- `gen/android/app/src/main/java/com/mapxr/app/ShizukuDispatcher.kt` — new; `ShizukuState`
  sealed class, `StateFlow<ShizukuState>`, full Shizuku lifecycle (init/bind/unbind/reconnect),
  `dispatch(actionsJson)` with complete action JSON → InputEvent table (Key/KeyChord/TypeString/
  MouseClick/MouseDoubleClick/MouseScroll/Macro/Vibrate), full key name→KEYCODE mapping table
- `gen/android/app/src/main/java/com/mapxr/app/ShizukuPlugin.kt` — new; Tauri plugin with
  `getShizukuState`, `requestShizukuPermission`, `openShizukuApp` commands
- `gen/android/app/src/main/java/com/mapxr/app/NativeBridge.kt` — removed `initDispatch`/
  `dispatchActions`; added `external fun registerShizukuDispatcher()`
- `gen/android/app/src/main/java/com/mapxr/app/MainActivity.kt` — swapped
  `ShellServerPlugin` → `ShizukuPlugin`; added `ShizukuDispatcher.init(this)` and
  `NativeBridge.registerShizukuDispatcher()`
- `gen/android/app/src/main/java/com/mapxr/app/MapxrForegroundService.kt` — updated
  notification keyboard status to read `ShizukuDispatcher.state` instead of ShellServerManager
- `gen/android/app/src/main/AndroidManifest.xml` — added `ShizukuProvider`
- `apps/desktop/src-tauri/src/android_jni.rs` — replaced `initDispatch` JNI function with
  `registerShizukuDispatcher`; renamed `NATIVE_BRIDGE_CLASS` → `SHIZUKU_DISPATCHER_CLASS`
- `apps/desktop/src-tauri/src/android_pump.rs` — renamed `dispatch_via_shell` →
  `dispatch_via_shizuku`; updated JNI call to `ShizukuDispatcher.dispatch()`
- `apps/desktop/src/lib/commands.ts` — removed shell server commands/types; added
  `ShizukuState` type + `getShizukuState`, `requestShizukuPermission`, `openShizukuApp`
- `apps/desktop/src/lib/android-bridge.ts` — updated architecture comment
- `apps/desktop/src/lib/components/ShizukuSetup.svelte` — new; 3-step wizard
  (Install → Start → Permit → Active); polls state every 1 s
- `apps/desktop/src/routes/settings/+page.svelte` — swapped `ShellServerSetup` →
  `ShizukuSetup`; updated state field names and badge labels

**Notes:**
- `cargo clippy -- -D warnings` clean; `cargo test --workspace` passes.
- Key mapping table: all VALID_KEYS covered. F13–F24 use integer literals (170+n)
  to avoid API 29+ lint. Brightness keys use integer literals (220/221, API 33).
- `dispatch_via_shizuku` calls `ShizukuDispatcher.dispatch` as a JNI static method on the
  Kotlin object companion — Kotlin objects expose `@JvmStatic` methods as static Java methods.
- Macro delays use `Thread.sleep` on the pump thread; acceptable for infrequent macro use.
- Mouse click/scroll injects at screen centre (touchscreen source); no cursor position available.

**Next:** Task 21.7 — manual test matrix on device

---

## 2026-03-23 — 21.2: Epic 20 removal, Gradle/AIDL, InputUserService

**Tasks completed:** 21.2
**Tasks in progress:** none

**Files changed:**

- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/AdbKey.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/AdbPairing.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/AdbConnection.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/ShellServerManager.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/ShellClientManager.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/ShellServerPlugin.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/ShellInputEvent.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/test/java/com/mapxr/app/AdbPairingTest.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/test/java/com/mapxr/app/ShellClientManagerTest.kt` — deleted
- `apps/desktop/src-tauri/gen/android/shell-server/` — deleted (entire module)
- `apps/desktop/src/lib/components/ShellServerSetup.svelte` — deleted
- `apps/desktop/src-tauri/gen/android/app/build.gradle.kts` — removed spake2/BouncyCastle deps
  and `preBuild` shell-server hook; added `dev.rikka.shizuku:api:13.1.5` and
  `dev.rikka.shizuku:provider:13.1.5`; enabled `aidl = true` in buildFeatures;
  updated packaging pickFirsts → excludes
- `apps/desktop/src-tauri/gen/android/build.gradle.kts` — removed jitpack repository
- `apps/desktop/src-tauri/gen/android/settings.gradle` — removed `:shell-server` module include
- `apps/desktop/src-tauri/build.rs` — replaced `SHELL_SERVER_COMMANDS`/`shellServer` plugin
  with `SHIZUKU_COMMANDS`/`shizuku` plugin
- `apps/desktop/src-tauri/capabilities/mobile.json` — replaced `shellServer:default` with
  `shizuku:default`
- `apps/desktop/src-tauri/gen/android/app/src/main/aidl/com/mapxr/app/IInputService.aidl` —
  new; AIDL interface with `injectKey`, `injectMotion`, `destroy`
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/InputUserService.kt` —
  new; Shizuku UserService running as shell uid; implements IInputService.Stub() using reflection
  to call InputManagerGlobal.injectInputEvent(); falls back to `input keyevent` for key events

**Notes:**
Remaining Epic 20 references in 8 files (MapxrForegroundService, MainActivity, NativeBridge,
android_jni.rs, android_pump.rs, commands.ts, android-bridge.ts, settings/+page.svelte) are
intentional — they will be updated in tasks 21.3–21.6.

**Next:** Task 21.3 — write `ShizukuDispatcher.kt` (ShizukuState sealed class, StateFlow,
  init/requestPermission/bind/unbind/dispatch with full action-JSON → InputEvent table)

---

## 2026-03-23 — Epic 20 abandoned; Epic 21 (Shizuku) planned

**Tasks completed:** 20.9 cancelled; 21.1 (spec draft — approved)
**Tasks in progress:** none

**Files changed:**

- `docs/adb-issues.md` — added Final Conclusions section documenting root cause and decision
- `docs/plan/implementation-plan.md` — marked Epic 20 superseded; added Epic 21 (Shizuku)
  with task 21.1 complete; updated current focus and dependency map
- `docs/spec/android-shizuku-spec.md` — new; full spec for Epic 21 (approved)
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/AdbPairing.kt` —
  diagnostic logging added during investigation (full PeerInfo base64 log)
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/AdbConnection.kt` —
  diagnostic logging added during investigation (getCertificateChain log)
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/ShellServerManager.kt` —
  diagnostic logging added during investigation (SSLHandshakeException cause chain)

**Notes:**
Manual testing of Epic 20 (task 20.9) was attempted. SPAKE2 pairing completes and the device
shows `mapxr@mapxr` in Paired Devices. The TLS connection to adbd fails with
`CERTIFICATE_VERIFY_FAILED`. `adb logcat -s adbd` showed "Invalid base64 key" for every entry
in `adb_keys` loaded from our pairings.

The base64 was verified correct on the PC: `base64 -d | wc -c` = 524, first 4 bytes =
`40 00 00 00` (numWords = 64), last 4 bytes = `01 00 01 00` (exponent = 65537). The key is
structurally identical to the PC's `adbkey.pub`. The PC's key is accepted; ours is not.
Root cause unknown — likely a Samsung/Android 16 (API 36) modification to adbd's key storage
or validation that cannot be debugged without root access to the device.

Decision: replace Epic 20 with Epic 21 (Shizuku). Shizuku provides the same
`InputManager.injectInputEvent()` as shell uid capability without requiring a full ADB
protocol reimplementation.

**Next:** Task 21.2 — begin Epic 21 implementation: delete Epic 20 code per spec §4 removal
checklist, add Shizuku Gradle dependencies, write `IInputService.aidl` and
`InputUserService.kt`

---

## 2026-03-22 — 20.8: ShellServerSetup wizard + Keyboard Mode Settings section

**Tasks completed:** 20.8
**Tasks in progress:** none

**Files changed:**

- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/ShellServerPlugin.kt` — new;
  Tauri plugin with 4 commands: `getServerState` (state + step + apiLevel), `startAdbPairing`
  (calls `AdbPairing.pair()` on background coroutine, then retries `ShellServerManager.start()`),
  `openDeveloperOptions`, `retryStartup`
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/MainActivity.kt` —
  registered `ShellServerPlugin` as plugin name `"shellServer"`
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/MapxrForegroundService.kt` —
  added keyboard status line to notification via `BigTextStyle`; reads
  `ShellServerManager.serverState.value` at notification build time
- `apps/desktop/src/lib/commands.ts` — added `getShellServerState`, `startAdbPairing`,
  `openDeveloperOptions`, `retryShellServerStartup`; exported `ShellServerState` type
- `apps/desktop/src/lib/components/ShellServerSetup.svelte` — new; 4-step modal wizard
  (prerequisites → wireless → pair → active); polls state every 1 s in waiting steps;
  auto-advances on state change; pairing form with port + code inputs
- `apps/desktop/src/routes/settings/+page.svelte` — added "Keyboard Mode" section
  (status badge + Set up/View button + `ShellServerSetup` modal); `refreshAndroidStatus`
  now also fetches shell server state

**Notes:**
- Notification keyboard status is read at build time (when BlePlugin refreshes the
  notification). It does not live-update on shell server state change. Acceptable for now.
- `ShellServerSetup.svelte` polls every 1 s, not on every render, so it won't hammer the
  plugin when the wizard is open and idle.
- The `"Unsupported"` state disables the "Set up" button in Settings since there's nothing
  the user can do on Android < 11.

**Next:** Task 20.9 — manual test on device: pairing flow, key injection in various apps,
  mouse actions, background injection, reboot reconnect, cargo clippy + test clean.

---

## 2026-03-22 — 20.7: accessibility rollback

**Tasks completed:** 20.7
**Tasks in progress:** none

**Files changed:**

- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/AccessibilityDispatcher.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/MapxrAccessibilityService.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/AccessibilityPlugin.kt` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/main/res/xml/accessibility_service_config.xml` — deleted
- `apps/desktop/src/lib/components/AccessibilitySetupPrompt.svelte` — deleted
- `apps/desktop/src-tauri/gen/android/app/src/main/AndroidManifest.xml` — removed
  `MapxrAccessibilityService` `<service>` block
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/MainActivity.kt` — removed
  `AccessibilityPlugin` registration and `registerDispatchCallback`; added `initDispatch()`
  and `ShellServerManager.start(this)`
- `apps/desktop/src-tauri/gen/android/app/src/main/java/com/mapxr/app/NativeBridge.kt` — replaced
  `external fun registerDispatchCallback()` with `external fun initDispatch()`; updated doc
- `apps/desktop/src-tauri/src/android_jni.rs` — complete rewrite: removed `DISPATCH_CLASS`,
  `dispatch_class()`, `registerDispatchCallback` JNI function; replaced with `initDispatch` JNI
  function that stores `JAVA_VM` + `NATIVE_BRIDGE_CLASS`; updated module doc
- `apps/desktop/src-tauri/src/android_pump.rs` — fixed stale comments referencing
  `registerDispatchCallback` and `AccessibilityDispatcher`
- `apps/desktop/src/routes/settings/+page.svelte` — removed Accessibility section, related
  state (`accessibilityEnabled`, `accessibilityPromptOpen`), `AccessibilitySetupPrompt` import,
  and `checkAccessibilityEnabled` import; simplified `refreshAndroidStatus`
- `apps/desktop/src/lib/components/AndroidOnboarding.svelte` — removed accessibility phase;
  onboarding now goes directly to the battery setup step
- `apps/desktop/src/routes/+layout.svelte` — updated onboarding trigger condition from
  `!accessibility_setup_done || !battery_setup_done` to just `!battery_setup_done`
- `apps/desktop/src/lib/commands.ts` — removed `checkAccessibilityEnabled` and
  `openAccessibilitySettings` functions
- `apps/desktop/src/lib/android-bridge.ts` — updated doc comment to reference shell server

**Notes:**
- §6.3 grep checks: all accessibility source-file checks pass. `JAVA_VM` intentionally
  remains (spec §4.5 says "retained for calling into Kotlin from the async pump thread").
  `dispatchActions` in Kotlin source files is the new shell-server method in `NativeBridge` /
  `ShellClientManager` — not the old Tauri command, which was already absent.
- Stale build artifacts under `gen/android/app/build/` still reference old files; these will
  be invalidated on the next `cargo tauri android build`.
- `accessibility_setup_done` field is left in `AndroidPreferences` type and Rust struct —
  no longer used by onboarding flow but harmless to keep for backwards compatibility with
  existing preferences files.

**Next:** Task 20.8 — `ShellServerSetup.svelte` 4-step pairing wizard; "Keyboard Mode"
  section in Settings; status line in foreground service notification.

---
