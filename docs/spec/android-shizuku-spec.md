# Android Shizuku Key Injection Spec — Epic 21

## 1. Purpose and motivation

Epic 20 attempted to gain `shell` uid access via a reimplemented ADB Wireless Debugging
client. After full implementation and debugging, the ADB approach was abandoned: Samsung's
modified adbd (confirmed on Android 16 / API 36) rejects keys stored by our pairing
handshake with "Invalid base64 key" even though the keys are structurally identical to the
PC's accepted keys. The root cause cannot be diagnosed without root access. See
`docs/adb-issues.md` for the full investigation record.

**Shizuku** provides the same capability — `InputManager.injectInputEvent()` as shell uid —
via a maintained library with a documented API. The goal of this epic is unchanged from
Epic 20:

| Action type        | Old path (Epic 15/19)            | New path (Epic 21)                    |
|--------------------|----------------------------------|---------------------------------------|
| Key / KeyChord     | `ACTION_SET_TEXT` (broken)       | `KeyEvent` via Shizuku UserService    |
| TypeString         | `ACTION_SET_TEXT` (broken)       | `KeyEvent` sequence via UserService   |
| Back / Home        | `performGlobalAction()`          | `KeyEvent(KEYCODE_BACK/HOME)`         |
| Volume / Media     | `AudioManager.dispatchMedia…()`  | `KeyEvent` via UserService            |
| MouseClick         | `GestureDescription` tap         | `MotionEvent` DOWN+UP via UserService |
| MouseScroll        | `GestureDescription` swipe       | `MotionEvent` swipe via UserService   |

AccessibilityService remains absent (removed in Epic 20.7). No accessibility permission
is required.

### What changes for the user

- Install **Shizuku** once (Play Store, F-Droid, or GitHub). ~6 MB.
- Start Shizuku once via Wireless Debugging (the same screen used in Epic 20's pairing
  wizard). After that, Shizuku auto-starts across reboots via Wireless Debugging.
- Grant mapxr permission in the Shizuku app (one tap, prompted automatically).

### What does NOT change

- Minimum Android version: 11 (API 30). Shizuku requires Android 6+ but the pump path
  requires API 30.
- Background dispatch works via the Epic 19 JNI path, unchanged.
- All action types from Epic 20 are supported identically.

---

## 2. Architecture overview

```
BLE bytes
    │
    ▼ (Kotlin BlePlugin.onTapBytes)
NativeBridge.processTapBytes()          ← JNI (Epic 19, unchanged)
    │
    ▼ (Rust android_pump.rs)
dispatch_via_shizuku(actions, jvm)      ← replaces dispatch_via_shell
    │
    ▼ (JNI call)
ShizukuDispatcher.dispatch(actionsJson) ← new Kotlin object
    │
    ▼
IInputService.injectKey(KeyEvent)       ← AIDL call to UserService
IInputService.injectMotion(MotionEvent) ←
    │
    ▼ (InputUserService — runs as shell uid, started by Shizuku)
InputManager.injectInputEvent()
```

The Rust pump and JNI bridge (`android_jni.rs`) change only in the name and target of the
dispatch callback. Everything upstream (BLE, engine, action resolution) is unchanged.

---

## 3. Dependencies

Add to `apps/desktop/src-tauri/gen/android/app/build.gradle.kts`:

```kotlin
implementation("dev.rikka.shizuku:api:13.1.5")
implementation("dev.rikka.shizuku:provider:13.1.5")
```

No other new dependencies. The `spake2-java` and BouncyCastle imports brought in by Epic 20
are removed along with the ADB code.

Confirm the exact latest stable release tag from `https://github.com/RikkaApps/Shizuku-API`
at implementation time; use the highest 13.x release available.

---

## 4. Epic 20 removal checklist

Delete these files entirely:

```
gen/android/app/src/main/java/com/mapxr/app/AdbKey.kt
gen/android/app/src/main/java/com/mapxr/app/AdbPairing.kt
gen/android/app/src/main/java/com/mapxr/app/AdbConnection.kt
gen/android/app/src/main/java/com/mapxr/app/ShellServerManager.kt
gen/android/app/src/main/java/com/mapxr/app/ShellClientManager.kt
gen/android/app/src/main/java/com/mapxr/app/ShellServerPlugin.kt
gen/android/app/src/main/java/com/mapxr/app/ShellInputEvent.kt
gen/android/shell-server/                                (entire module)
apps/desktop/src/lib/components/ShellServerSetup.svelte
```

Modify (do not delete):

| File | Change |
|------|--------|
| `gen/android/app/src/main/java/com/mapxr/app/NativeBridge.kt` | Remove `registerDispatchCallback()`; add `registerShizukuDispatcher()` (§7.3) |
| `gen/android/app/src/main/java/com/mapxr/app/MainActivity.kt` | Unregister `ShellServerPlugin`; register `ShizukuPlugin` (§7.4); call `ShizukuDispatcher.init()` |
| `gen/android/app/src/main/java/com/mapxr/app/MapxrForegroundService.kt` | Update notification status line to read `ShizukuDispatcher.state` instead of `ShellServerManager.serverState` |
| `apps/desktop/src-tauri/gen/android/app/build.gradle.kts` | Remove spake2 + extra BouncyCastle deps; add Shizuku deps |
| `apps/desktop/src-tauri/gen/android/build.gradle.kts` | Remove `shell-server` module include |
| `apps/desktop/src-tauri/gen/android/settings.gradle` | Remove `shell-server` module |
| `gen/android/app/src/main/AndroidManifest.xml` | Remove Wireless Debugging permission; add ShizukuProvider (§6.1) |
| `apps/desktop/src-tauri/src/android_jni.rs` | Replace `dispatch_via_shell` callback with `dispatch_via_shizuku` (§7.3) |
| `apps/desktop/src-tauri/src/android_pump.rs` | Replace `dispatch_via_shell` with `dispatch_via_shizuku` (§7.3) |
| `apps/desktop/src/lib/commands.ts` | Replace shell server commands with Shizuku commands (§9.1) |
| `apps/desktop/src/lib/android-bridge.ts` | No dispatch changes needed; update any state type imports |
| `apps/desktop/src/routes/settings/+page.svelte` | Replace "Keyboard Mode" section to use `ShizukuSetup` (§9.2) |

After completing the removal and all §7–9 additions, run:

```bash
grep -r "ShellServer\|ShellClient\|AdbKey\|AdbPairing\|AdbConnection\|ShellInput\|shell_server\|dispatch_via_shell" \
  apps/desktop/src-tauri/src apps/desktop/src-tauri/gen apps/desktop/src
```

This should return no results.

---

## 5. Shizuku setup states

`ShizukuDispatcher` exposes a `StateFlow<ShizukuState>`:

```kotlin
sealed class ShizukuState {
    /** Android < 11 or Shizuku API not available. */
    object Unsupported : ShizukuState()
    /** Shizuku app is not installed. */
    object NotInstalled : ShizukuState()
    /** Shizuku is installed but not running (user has not started it). */
    object NotRunning : ShizukuState()
    /** Shizuku is running but mapxr does not have permission yet. */
    object PermissionRequired : ShizukuState()
    /** Permission granted, binding UserService. */
    object Binding : ShizukuState()
    /** UserService bound and ready to inject. */
    object Active : ShizukuState()
    /** UserService was bound but disconnected unexpectedly; auto-rebind in progress. */
    object Reconnecting : ShizukuState()
}
```

State transitions:

```
Unsupported  ← API < 30 (permanent)
NotInstalled → (user installs Shizuku) → NotRunning
NotRunning   → (user starts Shizuku via Wireless Debugging) → PermissionRequired
PermissionRequired → (user grants) → Binding
Binding      → (service connected) → Active
Active       → (service disconnected) → Reconnecting → Binding → Active
```

---

## 6. AndroidManifest.xml changes

### 6.1 Add ShizukuProvider

```xml
<provider
    android:name="rikka.shizuku.ShizukuProvider"
    android:authorities="${applicationId}.shizuku"
    android:multiprocess="false"
    android:enabled="true"
    android:exported="true"
    android:permission="android.permission.INTERACT_ACROSS_USERS_FULL" />
```

### 6.2 Remove from Epic 20

Remove the following that were added for Epic 20 (if present):

- `android.permission.MANAGE_USB`
- Any `<service>` entries for `ShellServerPlugin` or similar
- Any `<uses-permission>` added solely for Wireless Debugging

---

## 7. Kotlin components

### 7.1 IInputService.aidl

Create `gen/android/app/src/main/aidl/com/mapxr/app/IInputService.aidl`:

```aidl
package com.mapxr.app;

interface IInputService {
    /**
     * Inject a KeyEvent. Runs as shell uid; caller must construct a valid KeyEvent.
     * eventTime and downTime should use SystemClock.uptimeMillis().
     */
    void injectKey(in KeyEvent event);

    /**
     * Inject a MotionEvent (click, scroll).
     * eventTime, downTime, and source must be set correctly by the caller.
     */
    void injectMotion(in MotionEvent event);

    /** Terminate the UserService process cleanly. */
    void destroy();
}
```

### 7.2 InputUserService.kt

Create `gen/android/app/src/main/java/com/mapxr/app/InputUserService.kt`.

This class is started by Shizuku as a separate process running as shell uid (2000).
Shell uid holds `android.permission.INJECT_EVENTS`, enabling `InputManager.injectInputEvent()`.

```kotlin
/**
 * Shizuku UserService running as shell uid.
 *
 * Started by Shizuku via [ShizukuDispatcher]. Exposes [IInputService] over Binder.
 * Uses reflection to call [android.hardware.input.InputManager.injectInputEvent] since
 * it is @hide but accessible from shell uid.
 *
 * Constructor signature must match what Shizuku expects: either no-arg or (IBinder).
 * We use the no-arg form; Shizuku will call [asBinder] to get the IBinder to return to
 * the client.
 */
class InputUserService : IInputService.Stub() {

    private val injectInputEvent: Method by lazy {
        // InputManagerGlobal.getInstance().injectInputEvent(InputEvent, int)
        // Available on API 26+. Falls back to InputManager.getInstance() on older APIs.
        val cls = try {
            Class.forName("android.hardware.input.InputManagerGlobal")
        } catch (_: ClassNotFoundException) {
            Class.forName("android.hardware.input.InputManager")
        }
        val getInstance = cls.getDeclaredMethod("getInstance").also { it.isAccessible = true }
        val instance = getInstance.invoke(null)
        cls.getDeclaredMethod("injectInputEvent", InputEvent::class.java, Int::class.java)
            .also { it.isAccessible = true }
            .let { method -> method.also { _instance = instance } }
    }

    private var _instance: Any? = null

    private fun inject(event: InputEvent) {
        injectInputEvent.invoke(_instance, event, 0 /* INJECT_INPUT_EVENT_MODE_ASYNC */)
    }

    override fun injectKey(event: KeyEvent) = inject(event)

    override fun injectMotion(event: MotionEvent) = inject(event)

    override fun destroy() = System.exit(0)
}
```

> **Note for implementation:** The reflection target (`InputManagerGlobal` vs `InputManager`)
> may need adjustment if it fails on the target API level. If both fail, fall back to
> `Runtime.getRuntime().exec(arrayOf("input", "keyevent", "$keyCode"))` as a last resort
> (works but has no timing guarantees). Log the fallback with a warning. Do not silently
> swallow exceptions.

### 7.3 ShizukuDispatcher.kt

Create `gen/android/app/src/main/java/com/mapxr/app/ShizukuDispatcher.kt`.

This is the Kotlin singleton called from JNI (replacing `ShellClientManager.sendActions`).
It owns the UserService connection lifecycle and converts action JSON to input events.

Responsibilities:

- Hold the `UserServiceArgs` and `ServiceConnection` for `InputUserService`
- Expose `state: StateFlow<ShizukuState>`
- `init(context)` — called from `MainActivity.onCreate`; registers Shizuku permission listener,
  checks initial state, starts binding if already permitted
- `requestPermission()` — calls `Shizuku.requestPermission(REQUEST_CODE)`
- `bind()` / `unbind()` — `Shizuku.bindUserService` / `Shizuku.unbindUserService`
- `dispatch(actionsJson: String)` — called from JNI; parses actions, calls
  `inputService?.injectKey(…)` / `inputService?.injectMotion(…)`; drops silently if
  `inputService` is null (graceful degradation — see §11)

**Action JSON → InputEvent mapping:**

Re-use the key name → `KeyEvent.KEYCODE_*` mapping table from the deleted
`AccessibilityDispatcher.kt`. The mapping is unchanged; only the injection call site differs.

Key event construction:
```kotlin
val now = SystemClock.uptimeMillis()
KeyEvent(now, now, KeyEvent.ACTION_DOWN, keyCode, 0, metaState,
    KeyCharacterMap.VIRTUAL_KEYBOARD, 0,
    KeyEvent.FLAG_FROM_SYSTEM or KeyEvent.FLAG_VIRTUAL_HARD_KEY,
    InputDevice.SOURCE_KEYBOARD)
```
Send both `ACTION_DOWN` and `ACTION_UP` for each key. For `KeyChord`, send all modifier
DOWN events, the primary key DOWN+UP, then all modifier UP events in reverse order.

Mouse click (`MotionEvent`):
```kotlin
val source = InputDevice.SOURCE_TOUCHSCREEN
MotionEvent.obtain(downTime, eventTime, action, x, y, 0).also {
    it.source = source
}
```
Send `ACTION_DOWN` then `ACTION_UP` at the same coordinate for a click.
For scroll, send `ACTION_SCROLL` with `setAxisValue(MotionEvent.AXIS_VSCROLL, delta)`.

`TypeString`: inject one `KeyEvent(ACTION_MULTIPLE, KEYCODE_UNKNOWN)` with the string as
`characters`. If `ACTION_MULTIPLE` fails on the target (some OEMs drop it), fall back to
decomposing the string into individual character key events via `KeyCharacterMap`.

### 7.4 ShizukuPlugin.kt

Create `gen/android/app/src/main/java/com/mapxr/app/ShizukuPlugin.kt` — Tauri plugin
exposing state and control to the Svelte UI.

Commands:

| Command | Description | Response |
|---------|-------------|----------|
| `getShizukuState` | Current `ShizukuState` as string | `{ state, apiLevel }` |
| `requestShizukuPermission` | Call `ShizukuDispatcher.requestPermission()` | resolves immediately; state update via polling |
| `openShizukuApp` | Launch Shizuku if installed; else open Play Store page | always resolves |

`getShizukuState` response shape:
```json
{
  "state": "Unsupported" | "NotInstalled" | "NotRunning" |
           "PermissionRequired" | "Binding" | "Active" | "Reconnecting",
  "apiLevel": 36
}
```

Register `ShizukuPlugin` in `MainActivity.kt` with plugin name `"shizuku"`.
Remove `ShellServerPlugin` registration.

### 7.5 NativeBridge.kt changes

Remove:
```kotlin
external fun registerDispatchCallback()
```

Add:
```kotlin
/** Called from MainActivity after ShizukuDispatcher.init(); stores JVM + dispatcher ref
 *  so android_jni.rs can call ShizukuDispatcher.dispatch() without a Tauri context. */
external fun registerShizukuDispatcher()
```

---

## 8. Rust / JNI changes

### 8.1 android_jni.rs

Replace the `dispatch_via_shell` JNI callback with `dispatch_via_shizuku`:

```rust
/// Called from android_pump.rs to dispatch resolved actions.
/// Finds the ShizukuDispatcher class and calls dispatch(actionsJson).
#[cfg(target_os = "android")]
pub fn dispatch_via_shizuku(actions_json: &str) {
    // attach JVM, find ShizukuDispatcher class, call dispatch(actionsJson)
    // same pattern as the former dispatch_via_shell / registerDispatchCallback
}

/// JNI entry point called from NativeBridge.registerShizukuDispatcher().
/// Stores JavaVM + GlobalRef to ShizukuDispatcher class for use in dispatch_via_shizuku.
#[no_mangle]
#[cfg(target_os = "android")]
pub extern "C" fn Java_com_mapxr_app_NativeBridge_registerShizukuDispatcher(
    env: JNIEnv,
    _class: JClass,
) { /* ... */ }
```

### 8.2 android_pump.rs

Replace:
```rust
dispatch_via_shell(&actions_json, &jvm, &dispatch_class);
```
With:
```rust
dispatch_via_shizuku(&actions_json);
```

No change to the pump logic, action serialisation, or event loop.

---

## 9. Svelte / TypeScript changes

### 9.1 commands.ts

Remove:
- `getShellServerState`
- `startAdbPairing`
- `openDeveloperOptions`
- `retryShellServerStartup`
- `ShellServerState` type

Add:
```typescript
export type ShizukuState =
  | 'Unsupported' | 'NotInstalled' | 'NotRunning'
  | 'PermissionRequired' | 'Binding' | 'Active' | 'Reconnecting';

/** Returns the current Shizuku integration state. */
export async function getShizukuState(): Promise<{ state: ShizukuState; apiLevel: number }>;

/** Trigger the Shizuku permission request dialog. */
export async function requestShizukuPermission(): Promise<void>;

/** Open the Shizuku app, or the Play Store listing if not installed. */
export async function openShizukuApp(): Promise<void>;
```

### 9.2 ShizukuSetup.svelte

Create `apps/desktop/src/lib/components/ShizukuSetup.svelte` — replaces
`ShellServerSetup.svelte`. A 3-step modal wizard:

| Step | Condition | User action |
|------|-----------|-------------|
| **1 — Install** | `state === 'NotInstalled'` | "Open Play Store" button → `openShizukuApp()`; "Installed" button advances |
| **2 — Start** | `state === 'NotRunning'` | Instructions: open Shizuku → tap "Start via Wireless Debugging"; polls state every 1 s |
| **3 — Permit** | `state === 'PermissionRequired'` | "Grant permission" button → `requestShizukuPermission()`; polls state every 1 s |
| **Active** | `state === 'Active'` | Close button; show green status badge |

States `Binding` and `Reconnecting` show a spinner within whichever step is current.
State `Unsupported` disables the "Set up" button in Settings with tooltip "Requires Android 11+".

Poll interval: 1 s (same as `ShellServerSetup`). Stop polling when modal closes.

### 9.3 settings/+page.svelte

The "Keyboard Mode" section already exists from Epic 20.8. Change it to:
- Import `ShizukuSetup` instead of `ShellServerSetup`
- Call `getShizukuState` instead of `getShellServerState` in `refreshAndroidStatus`
- Status badge labels: "Active" (green), "Not running" (yellow), "Not installed" (red),
  "Unsupported" (grey)

---

## 10. Reboot persistence

Shizuku started via Wireless Debugging auto-starts on reboot as long as Wireless Debugging
remains enabled in Developer Options. No user action is needed. This is equivalent to the
shell server reconnect behaviour from Epic 20.

`ShizukuDispatcher.init()` is called every time `MainActivity.onCreate()` runs. If Shizuku
is already running and permission is already granted, it immediately transitions
`NotRunning → PermissionRequired → Binding → Active` within a few hundred milliseconds.
The BLE pump does not need to wait for this: if `inputService` is null during the bind,
events are dropped (§11). Once `Active`, all subsequent events are injected.

---

## 11. Graceful degradation

When `ShizukuDispatcher.dispatch()` is called but `inputService` is null (any non-Active state):
- Log a warning at `Log.w` level: `"Shizuku not active — dropping N actions"`
- Do NOT throw or surface an error to the user during normal operation
- The Settings badge shows the non-Active state so the user can investigate

If the UserService disconnects unexpectedly while `Active`:
- Set state to `Reconnecting`
- Attempt `Shizuku.bindUserService` again after 1 s
- If 3 consecutive bind attempts fail (e.g. Shizuku was killed), set state to `NotRunning`
  and require user to restart Shizuku

---

## 12. Testing (task 21.N — final task)

Manual test matrix (same as Epic 20.9 modulo setup path):

- [ ] Fresh install: Shizuku not installed → wizard shows Install step → install → Start step
      → start via Wireless Debugging → Permit step → grant → Active badge in Settings
- [ ] Key injection: terminal, Chrome address bar, message app — character appears
- [ ] KeyChord: Ctrl+A selects all in a text app
- [ ] Global shortcut: Home, Back, Recents work
- [ ] Volume / media keys fire
- [ ] TypeString: types multi-character string correctly
- [ ] Mouse click: taps foreground coordinate
- [ ] Mouse scroll: scrolls a list
- [ ] Background injection: MapXr minimised → tap → key appears in foreground app
- [ ] Reboot: device reboots → Shizuku auto-starts → MapXr opens → Active within 2 s
- [ ] Graceful degradation: Shizuku killed → Settings shows "Not running" → injection drops
      silently → restart Shizuku → Active resumes
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo test --workspace` passes

---

## 13. File layout after Epic 21

New files:
```
gen/android/app/src/main/aidl/com/mapxr/app/IInputService.aidl
gen/android/app/src/main/java/com/mapxr/app/InputUserService.kt
gen/android/app/src/main/java/com/mapxr/app/ShizukuDispatcher.kt
gen/android/app/src/main/java/com/mapxr/app/ShizukuPlugin.kt
apps/desktop/src/lib/components/ShizukuSetup.svelte
```

Deleted files (full list in §4):
```
gen/android/app/src/main/java/com/mapxr/app/AdbKey.kt
gen/android/app/src/main/java/com/mapxr/app/AdbPairing.kt
gen/android/app/src/main/java/com/mapxr/app/AdbConnection.kt
gen/android/app/src/main/java/com/mapxr/app/ShellServerManager.kt
gen/android/app/src/main/java/com/mapxr/app/ShellClientManager.kt
gen/android/app/src/main/java/com/mapxr/app/ShellServerPlugin.kt
gen/android/app/src/main/java/com/mapxr/app/ShellInputEvent.kt
gen/android/shell-server/
apps/desktop/src/lib/components/ShellServerSetup.svelte
```
