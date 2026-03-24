# Android Shell Server Spec — Epic 20

## 1. Purpose and motivation

The Epic 15/19 AccessibilityService approach for key injection (`ACTION_SET_TEXT` on
`AccessibilityNodeInfo`) is fundamentally limited: it performs content surgery on text fields rather
than injecting real `KeyEvent` objects. It fails in browser address bars, games, global shortcuts,
and any context without a focused accessible input node. The official Tap app provides this same
limited approach. For MapXr to differentiate itself, it must provide true keyboard emulation.

`InputManager.injectInputEvent()` is the Android system API for injecting `KeyEvent` and
`MotionEvent` objects identically to a physical USB or Bluetooth keyboard or touchscreen. It
requires `android.permission.INJECT_EVENTS`, a `signature`-level permission unavailable to
normal apps. However, the `shell` user (uid 2000) holds this permission — this is how
`adb shell input keyevent`, `adb shell input tap`, and `adb shell input swipe` all work.

This epic starts a persistent Java server process running as the `shell` user on the device via
the Android Wireless Debugging ADB interface. MapXr communicates with the server over a Unix
domain socket to inject arbitrary `KeyEvent` and `MotionEvent` objects. No external device, PC,
or third-party app is required after a one-time pairing setup.

`InputManager.injectInputEvent()` from the shell uid can replace **everything** the
AccessibilityService was used for in MapXr:

| Action type | Old path | New path |
|---|---|---|
| Key / KeyChord | `ACTION_SET_TEXT` (broken) | `KeyEvent` via shell server |
| TypeString | `ACTION_SET_TEXT` (broken) | `KeyEvent` sequence via shell server |
| Back / Home / Recents | `performGlobalAction()` | `KeyEvent(KEYCODE_BACK/HOME/APP_SWITCH)` |
| Volume / Media | `AudioManager.dispatchMediaKeyEvent()` | `KeyEvent` via shell server |
| MouseClick | `GestureDescription` tap | `MotionEvent` DOWN+UP via shell server |
| MouseDoubleClick | `GestureDescription` double tap | `MotionEvent` double tap via shell server |
| MouseScroll | `GestureDescription` swipe | `MotionEvent` swipe via shell server |

The AccessibilityService is therefore **removed entirely**. No accessibility permission is
required. The onboarding flow is simplified to a single setup path.

### What this provides

- Injection identical to a physical Bluetooth keyboard and touchscreen — apps cannot distinguish it
- Works in all contexts: text fields, browser address bars, games, global shortcuts, terminal apps,
  gesture navigation
- Works when MapXr is backgrounded (JNI pump path from Epic 19, unchanged)
- Single app, no Shizuku or other dependency, no accessibility permission
- Android 11+ (Wireless Debugging required); Android 10 and below: all dispatch actions are
  dropped with a user-visible warning — see §3

### What this does NOT provide

- Any dispatch on Android 10 or below
- Injection into `FLAG_SECURE` windows (lock screen, some banking apps) — same limitation as
  physical hardware input
- Persistence across reboots without a reconnect step (shell processes do not survive reboot;
  MapXr reconnects automatically in ~2 seconds on next open, no user action needed)

---

## 2. Architecture overview

```
Tap Strap BLE
  │
  ▼
BlePlugin.onTapBytes()
  │  NativeBridge.processTapBytes()  ← JNI (android_jni.rs, unchanged from Epic 19)
  ▼
run_android_pump  ←  mpsc::Receiver<TapEventMsg>
  │  process_android_outputs() resolves Vec<Action>
  ▼
dispatch_via_shell()                               [replaces dispatch_via_jni]
  └─ all Action types
       └─ ShellClientManager.send(ShellInputEvent) ← Unix socket (@mapxr_input)
            └─ MapxrShellServer (shell uid)
                 └─ InputManager.injectInputEvent(KeyEvent | MotionEvent)

WebView path (debug only, unchanged):
  app.emit("tap-actions-fired") → debug panel event log
  app.emit("layer-changed")     → layer indicator
```

There is no longer any path through the AccessibilityService for dispatch.

---

## 3. Android version matrix

| Android version | API | Shell server | All dispatch |
|---|---|---|---|
| 11+ | 30+ | ✅ Wireless Debugging available | ✅ Via shell server |
| 10 | 29 | ❌ No Wireless Debugging | ❌ All actions dropped |
| ≤ 9 | ≤ 28 | ❌ | ❌ All actions dropped |

On Android 10 and below, MapXr logs a warning for every dropped action and shows a persistent
Settings banner: "MapXr requires Android 11 or later for key injection. Tap gestures are
detected but no actions will be sent to other apps." This covers less than 5% of active Android
devices as of 2026.

---

## 4. Components

### 4.1 MapxrShellServer (server-side, runs as shell uid)

A standalone Kotlin class compiled to a separate DEX file and bundled in
`apps/desktop/src-tauri/gen/android/app/src/main/assets/mapxr-shell-server.dex`.

**Startup:** MapXr deploys the DEX to `/data/local/tmp/mapxr-shell-server.dex` via the ADB
session, then runs:

```
app_process -Djava.class.path=/data/local/tmp/mapxr-shell-server.dex \
            /data/local/tmp \
            com.mapxr.shellserver.MapxrShellServer
```

**Protocol:** The server binds a Unix domain socket on the abstract name `@mapxr_input`. Each
message is a length-prefixed (4-byte big-endian `int32`) payload — see §5.3 for message format.

**Key injection:**

```kotlin
val im = Class.forName("android.hardware.input.InputManager")
    .getMethod("getInstance").invoke(null)
val injectMethod = im.javaClass.getMethod(
    "injectInputEvent",
    android.view.InputEvent::class.java,
    Int::class.javaPrimitiveType
)
// For key events:
injectMethod.invoke(im, keyEvent, 0 /* INJECT_INPUT_EVENT_MODE_ASYNC */)
// For motion events:
injectMethod.invoke(im, motionEvent, 0)
```

Reflection is used because `InputManager.getInstance()` and `injectInputEvent()` are hidden
APIs. This path is stable across Android 11–15 and is used by Shizuku and KeyMapper in
production. The DEX runs as shell uid, so the `INJECT_EVENTS` permission check passes.

**Gesture construction:** For high-level gesture messages (tap, swipe), the server constructs
the full `MotionEvent` sequence internally:
- `TAP(x, y)` → `ACTION_DOWN` at t=0 then `ACTION_UP` at t=50ms, source=`SOURCE_TOUCHSCREEN`
- `DOUBLE_TAP(x, y)` → two tap sequences separated by 100ms
- `SWIPE(x1, y1, x2, y2, durationMs)` → `ACTION_DOWN` then a series of `ACTION_MOVE` events
  interpolated along the path then `ACTION_UP`
- Screen centre coordinates are obtained server-side via `WindowManager.getDefaultDisplay()`
  (deprecated but works on all target API levels; use `WindowMetrics` on API 30+)

**Liveness:** The server emits a heartbeat byte `0x01` every 5 seconds to any connected client.
If the client disconnects, the server continues listening for a new connection. The server does
not exit unless explicitly killed or the device reboots.

**Build:** A separate Gradle module `apps/desktop/src-tauri/gen/android/shell-server/` produces
the DEX. The main app's Gradle build depends on this module so the DEX is always current. The
DEX is a raw asset; Gradle does not dex-merge it into the main APK classes.

### 4.2 ADB client module (Kotlin, runs in MapXr app process)

Handles establishing and maintaining the ADB session used to deploy and start the shell server.

**References:**
- LADB (GitHub: tytydraco/LADB, Apache 2.0) — complete Android-native ADB TLS + SPAKE2+
  implementation in Kotlin; primary implementation reference
- dadb (Maven: `com.github.mobile-dev-inc:dadb`, Apache 2.0) — JVM ADB client library;
  evaluate for Android compatibility in task 20.2 before adopting

**Responsibilities:**
1. Discover the Wireless Debugging port via `Settings.Global.getString(cr, "adb_wifi_port")`
   (readable without special permission on Android 11+)
2. Manage the RSA keypair stored in Android Keystore (`MapxrAdbKey`) — generated once at
   pairing time, survives reboots, never leaves the device
3. Implement TLS 1.3 ADB connection using the stored keypair
4. Implement the one-time SPAKE2+ pairing flow (see §5.1)
5. Open a shell channel on the established session
6. Deploy `mapxr-shell-server.dex` to `/data/local/tmp/` if absent or version-mismatched
7. Start the shell server via `app_process`
8. Detect session loss and reconnect (no re-pairing needed after first setup)

The ADB client runs on a background coroutine and does not block the Tauri UI thread.

### 4.3 ShellClientManager (Kotlin, in MapXr app process)

Manages the Unix socket connection from the MapXr app process to the running server.

```kotlin
object ShellClientManager {
    fun send(event: ShellInputEvent): Boolean  // false if server not reachable
    fun isConnected(): Boolean
    fun connect()
    fun disconnect()
}
```

`send()` is called from the Rust pump dispatch path via JNI. If the socket is not connected,
`send()` queues up to 16 pending events and triggers an async reconnect; queued events are
flushed on reconnect or dropped after 2 seconds. This prevents blocking the pump thread.

### 4.4 Server lifecycle management (Kotlin)

`ShellServerManager` orchestrates the full lifecycle:

```
app start
  │
  ├─ Android < 11? → show "requires Android 11+" banner, stop
  │
  ├─ shell server socket reachable (@mapxr_input)?
  │    YES → connect ShellClientManager, done
  │    NO  ↓
  ├─ Wireless Debugging port open (adb_wifi_port non-zero)?
  │    NO  → show "open Wireless Debugging" prompt in Settings, stop
  │    YES ↓
  ├─ ADB session established with stored keypair?
  │    NO  → keypair absent or rejected → show pairing UI
  │    YES ↓
  ├─ deploy DEX if absent or version-mismatched
  ├─ start shell server via app_process
  ├─ wait for socket (up to 3s, 500ms poll)
  └─ connect ShellClientManager
```

**Post-reboot:** The ADB keypair survives in Android Keystore. adbd retains trusted keys across
reboots. The socket is gone but reconnect + server restart complete in ~2 seconds on next
MapXr open with no user action required.

**Status notification:** A line in the foreground service notification shows the shell server
state: "Keyboard: active", "Keyboard: reconnecting…", or "Keyboard: setup needed". Tapping
opens MapXr Settings.

### 4.5 Rust pump integration

`dispatch_via_jni` in `android_pump.rs` is replaced by `dispatch_via_shell`, which routes all
dispatchable actions to `ShellClientManager` via a new JNI entry point:

```rust
#[cfg(target_os = "android")]
fn dispatch_via_shell(actions_json: &str) {
    // Call Java_com_mapxr_app_NativeBridge_dispatchActions via stored JAVA_VM
    // ShellClientManager.sendActions(actionsJson) on the Kotlin side translates
    // each Action to the appropriate ShellInputEvent and calls send().
}

#[cfg(not(target_os = "android"))]
fn dispatch_via_shell(_: &str) {}
```

`ShellClientManager.sendActions(actionsJson: String)` parses the JSON and converts each action:

| Action | ShellInputEvent |
|---|---|
| `Key { key, modifiers }` | `KeyDown(keycode, meta)` + `KeyUp(keycode, meta)` |
| `KeyChord { keys }` | modifier `KeyDown` events, then key `KeyDown`+`KeyUp`, then modifier `KeyUp` events |
| `TypeString { text }` | sequence of `KeyDown`+`KeyUp` per character, using `KeyCharacterMap` |
| `MouseClick { button }` | `Tap(cx, cy)` where cx/cy = screen centre |
| `MouseDoubleClick { button }` | `DoubleTap(cx, cy)` |
| `MouseScroll { direction }` | `Swipe(cx, cy, dx, dy, 200ms)` |
| `Vibrate` | not sent to server; handled separately (future task) |
| Back / Home / Recents (via `Key`) | `KeyDown`+`KeyUp` with `KEYCODE_BACK/HOME/APP_SWITCH` |

The `JAVA_VM` static in `android_jni.rs` is retained (needed to call into Kotlin from the
async pump thread). `DISPATCH_CLASS` is replaced with a reference to `NativeBridge` or
`ShellClientManager` as appropriate.

### 4.6 Setup UI

`ShellServerSetup.svelte` — a 4-step wizard in Settings → Keyboard Mode:

**Step 1 — Prerequisites:**
> "Full keyboard mode requires Android 11 and Developer Options."
> ✓ Android 11+ detected / ✗ Android 10 — not supported
> ✓ Developer Options enabled / [How to enable]

**Step 2 — Wireless Debugging:**
> "Enable Wireless Debugging in Developer Options."
> [Open Developer Options]
> Status polls `adb_wifi_port` every second; advances automatically when non-zero.

**Step 3 — Pair:**
> "In Wireless Debugging, tap 'Pair device with pairing code'."
> "Enter the port and 6-digit code shown by Android:"
> [Port ____] [Code ______] [Pair]

**Step 4 — Active:**
> "✓ Full keyboard mode is active."
> "After a restart, MapXr reconnects automatically when you open the app."

If the shell server is unavailable at action dispatch time, MapXr shows a snackbar:
"Keyboard mode not active — open Settings to reconnect."

---

## 5. Protocol details

### 5.1 Pairing protocol (SPAKE2+, one-time)

Android 11+ Wireless Debugging uses RFC 9382 SPAKE2+ over TLS 1.3.

1. User opens "Pair device with pairing code" in Developer Options → Wireless Debugging
2. Android starts a temporary TLS server on a random pairing port and shows a 6-digit code
3. MapXr connects to `127.0.0.1:<pairing_port>` with TLS 1.3 (accept any server cert)
4. Inside the TLS tunnel, SPAKE2+ runs using the 6-digit code as the shared password
5. Both sides exchange `PeerInfo` containing their RSA public key
6. adbd adds MapXr's public key to its trusted list; MapXr stores the keypair in Android Keystore
7. Pairing port closes; connection to the regular ADB port is now possible with the stored key

**Crypto requirements:**
- SPAKE2+ — Bouncy Castle 1.70+ (`org.bouncycastle:bcprov-jdk18on`)
- RSA 2048 generation and storage — Android Keystore (`KeyPairGenerator` with `AndroidKeyStore`)
- TLS 1.3 — standard `SSLContext` with a custom `X509TrustManager` that accepts self-signed certs

### 5.2 ADB connection protocol

After pairing, connections use mutual TLS with the stored RSA keypair:

1. TLS 1.3 to `127.0.0.1:<adb_wifi_port>` presenting the MapXr RSA certificate
2. Send ADB `CNXN` packet (`host:mapxr:1`)
3. Receive server `CNXN` — session is established as shell uid
4. Open shell channel (`OPEN` with `shell:`) for DEX deployment and server startup
5. Keepalive coroutine sends a no-op every 30 seconds to prevent adbd idle timeout

### 5.3 Shell server IPC protocol

Unix abstract socket `@mapxr_input` (no filesystem entry; kernel-managed).

Frame format (app → server):

```
[4 bytes big-endian int32: payload length N]
[1 byte: message type]
[N-1 bytes: message body]
```

Message types:

**0x01 — KeyEvent**
```
keycode:    int32  (KeyEvent.KEYCODE_*)
action:     int8   (0=DOWN, 1=UP)
meta_state: int32  (META_CTRL_ON etc.)
```
MapXr sends ACTION_DOWN immediately followed by ACTION_UP for each logical keypress.

**0x02 — Tap**
```
x: float32  (pixels; NaN = use screen centre)
y: float32  (pixels; NaN = use screen centre)
```
Server generates DOWN at t=0 then UP at t+50ms with `SOURCE_TOUCHSCREEN`.

**0x03 — DoubleTap**
```
x: float32
y: float32
```
Server generates two Tap sequences 150ms apart.

**0x04 — Swipe**
```
x1: float32   start (NaN = screen centre)
y1: float32
x2: float32   end   (NaN = screen centre + direction offset)
y2: float32
duration_ms: int32
```
Server generates DOWN, interpolated MOVE events at ~60fps, then UP.

**0x05 — Heartbeat request** (no body)
Server replies with `0x01` heartbeat byte immediately. Client uses this to verify liveness
in addition to the server's 5-second unsolicited heartbeat.

Server → client heartbeat: single byte `0x01` every 5 seconds. Client triggers reconnect if
no heartbeat received within 15 seconds.

---

## 6. Rollback: removing all AccessibilityService code

`InputManager.injectInputEvent()` from the shell uid covers every dispatch case that the
AccessibilityService was used for. No accessibility permission is needed. The entire
AccessibilityService integration is deleted.

### 6.1 Files deleted entirely

| File | Reason |
|---|---|
| `AccessibilityDispatcher.kt` | Key/gesture dispatch logic; entirely superseded by shell server |
| `MapxrAccessibilityService.kt` | Service implementation; no longer needed for any dispatch |
| `AccessibilityPlugin.kt` | `checkAccessibilityEnabled`, `openAccessibilitySettings`; no longer needed |
| `AccessibilitySetupPrompt.svelte` | Accessibility setup prompt; no longer needed |
| `res/xml/accessibility_service_config.xml` | Service configuration |

### 6.2 Files modified

**`AndroidManifest.xml`:**
- Remove `<service android:name=".MapxrAccessibilityService" ...>` declaration
- Remove `<uses-permission android:name="android.permission.BIND_ACCESSIBILITY_SERVICE" />`

**`MainActivity.kt`:**
- Remove `pluginManager.load(null, "accessibility", AccessibilityPlugin(this), "{}")`
- Remove `NativeBridge.registerDispatchCallback()` call
- Keep: BlePlugin and BatteryPlugin registration, `super.onCreate()`

**`NativeBridge.kt`:**
- Remove `external fun registerDispatchCallback()`
- Keep: `external fun processTapBytes(...)`, `System.loadLibrary("mapxr_lib")`

**`BlePlugin.kt`:**
- No changes needed (already calls `NativeBridge.processTapBytes` and `trigger(...)`)

**`android_jni.rs`:**
- Remove: `JAVA_VM`, `DISPATCH_CLASS`, `java_vm()`, `dispatch_class()` statics and accessors
- Remove: `Java_com_mapxr_app_NativeBridge_registerDispatchCallback` exported function
- Keep: `APP_STATE`, `APP_HANDLE`, `init()`, `app_state()`, `app_handle()`
- Keep: `Java_com_mapxr_app_NativeBridge_processTapBytes` (unchanged)
- Add: `Java_com_mapxr_app_NativeBridge_dispatchActions` (new, calls `ShellClientManager`)

**`android_pump.rs`:**
- Replace `dispatch_via_jni` with `dispatch_via_shell` (calls `NativeBridge.dispatchActions`)
- Remove: `jni::objects::JValue` import, `jni_str!`/`jni_sig!` macro usage in dispatch
- Keep: `JAVA_VM` static reference for calling into Kotlin from the async pump thread
- Keep: `tap-actions-fired` WebView emit (debug panel), `layer-changed` emit (unchanged)

**`android-bridge.ts`:**
- No changes needed

**`commands.rs` (Tauri commands):**
- Remove: `dispatchActions` command if still present (was the pre-Epic-19 WebView path;
  verify with grep — may already have been removed in Epic 19)
- Keep: all other commands unchanged

### 6.3 Rollback verification checklist

- [ ] `cargo clippy -- -D warnings` — no warnings
- [ ] `cargo test --workspace` — all pass
- [ ] `grep -r "AccessibilityService\|AccessibilityPlugin\|AccessibilityDispatcher" apps/desktop/src-tauri/gen/android/app/src/main/java/` — no results
- [ ] `grep -r "accessibility_service_config\|BIND_ACCESSIBILITY_SERVICE" apps/desktop/src-tauri/gen/android/` — no results
- [ ] `grep -r "registerDispatchCallback\|DISPATCH_CLASS\|JAVA_VM" apps/desktop/src-tauri/src/` — no results
- [ ] `grep -r "dispatchActions\|injectKey\|injectText\|ACTION_SET_TEXT" apps/desktop/src-tauri/gen/` — no results
- [ ] `grep -r "AccessibilitySetupPrompt" apps/desktop/src/` — no results
- [ ] `grep -r "checkAccessibilityEnabled\|openAccessibilitySettings" apps/desktop/src/` — no results
- [ ] Build installs and runs without crash; Settings page has no accessibility section

---

## 7. Implementation tasks

### 20a — Shell server

- [ ] **20.1** _(spec §4.1)_ Create `apps/desktop/src-tauri/gen/android/shell-server/` Gradle
  module. Write `MapxrShellServer.kt`: binds `@mapxr_input` abstract Unix socket, reads
  length-prefixed messages (§5.3), dispatches `KeyEvent` and `MotionEvent` via
  `InputManager.injectInputEvent()` reflection, constructs gesture `MotionEvent` sequences for
  Tap/DoubleTap/Swipe, emits 5-second heartbeat. Gradle task compiles to standalone DEX, copies
  to `app/src/main/assets/mapxr-shell-server.dex`.

### 20b — ADB client

- [ ] **20.2** _(spec §4.2, §5.2)_ Evaluate dadb for Android compatibility. Add as dependency
  or adapt LADB source. Implement `AdbConnection.kt`: TLS 1.3 connect using stored RSA keypair,
  ADB CNXN handshake, shell channel open, keepalive coroutine.

- [ ] **20.3** _(spec §5.1)_ Implement `AdbPairing.kt`: TLS connect to pairing port, SPAKE2+
  using Bouncy Castle, `PeerInfo` exchange, RSA keypair generation and Android Keystore storage.
  Unit tests with mock TLS server.

### 20c — Lifecycle and IPC

- [ ] **20.4** _(spec §4.4)_ Write `ShellServerManager.kt`: version check (Android 11+),
  startup orchestration, DEX deployment, server process start, socket polling.
  `serverState: StateFlow<ServerState>` for UI. Post-reboot auto-reconnect.

- [ ] **20.5** _(spec §4.3, §5.3)_ Write `ShellClientManager.kt`: Unix socket client,
  `sendActions(actionsJson)` parses actions and converts to `ShellInputEvent` messages, 16-event
  queue, heartbeat monitoring, async reconnect. Unit tests with local socket pair.

### 20d — Rust pump integration and rollback

- [ ] **20.6** _(spec §4.5)_ Add `Java_com_mapxr_app_NativeBridge_dispatchActions` JNI
  function calling `ShellClientManager.sendActions()`. Replace `dispatch_via_jni` in
  `android_pump.rs` with `dispatch_via_shell`. Update `android_jni.rs` statics accordingly.

- [ ] **20.7** _(spec §6)_ Execute rollback: delete `AccessibilityDispatcher.kt`,
  `MapxrAccessibilityService.kt`, `AccessibilityPlugin.kt`, `AccessibilitySetupPrompt.svelte`,
  `accessibility_service_config.xml`; update `AndroidManifest.xml`, `MainActivity.kt`,
  `NativeBridge.kt`, `android_jni.rs`, `android_pump.rs`; remove `dispatchActions` Tauri
  command if present. Verify all items in §6.3 checklist pass.

### 20e — Setup UI

- [ ] **20.8** _(spec §4.6)_ Write `ShellServerSetup.svelte` with 4-step pairing wizard. Add
  "Keyboard Mode" section to Android Settings page, bound to `serverState`. Add status line to
  foreground service notification. Remove accessibility section from Settings page.

### 20f — Testing

- [ ] **20.9** _(spec §8)_ Manual test on device (Pixel, Android 14 minimum; Samsung One UI
  secondary):
  - Pairing flow completes on-device without PC
  - Key injection in: terminal (Termux), Chrome address bar, message app, game (WASD), global
    shortcut (e.g. Gmail compose shortcut)
  - Mouse actions (click, scroll) via `MotionEvent` injection
  - Back/Home via `KEYCODE_BACK`/`KEYCODE_HOME`
  - App backgrounded: all actions arrive in foreground app
  - Reboot: reconnects automatically, no user action required
  - No accessibility permission granted: app functions fully
  - Shell server not running: snackbar shown, no crash
  - `cargo clippy -- -D warnings` and `cargo test --workspace` clean

---

## 8. Dependencies and risks

### New dependencies

| Library | Purpose | License | Decision |
|---|---|---|---|
| `org.bouncycastle:bcprov-jdk18on:1.78` | SPAKE2+ crypto | MIT | Add |
| dadb (TBD) | ADB protocol client | Apache 2.0 | Evaluate in 20.2; fall back to LADB adaptation if Android-incompatible |

### Removed dependencies (from rollback)

`AccessibilityService` framework usage is removed. No library dependency existed; the removal
reduces the app's declared permissions and eliminates the accessibility service declaration
from `AndroidManifest.xml`.

### Risks

**adbd API stability** — SPAKE2+ pairing and TLS ADB have been stable since Android 11, but
are undocumented. LADB and Shizuku changelogs document what changed in Android 12 and 13 and
are good reference points. Mitigation: test on API 30, 31, 33, 34, 35 in task 20.9.

**Google Play policy** — `InputManager.injectInputEvent()` via reflection may be flagged.
The use case is legitimate (keyboard emulation for a wearable input device) and Shizuku,
KeyMapper, and LADB are all on the Play Store using the same mechanism. Maintain a clear
privacy policy statement.

**Hidden API restrictions on the server** — Android 9+ restricts hidden API access in
installed APKs. The shell server runs as a standalone `app_process` invocation, not as a
sandboxed APK, so it may not be subject to the same restrictions. Shizuku uses this exact
pattern successfully. Verify empirically in task 20.1.

**`KEYCODE_HOME` on OEM skins** — `adb shell input keyevent KEYCODE_HOME` works on all
standard Android builds. Some Samsung/Xiaomi builds intercept HOME differently. Test
explicitly in 20.9; document any OEM-specific workarounds.

**Wireless Debugging port timing** — `adb_wifi_port` may be zero briefly after enabling
Wireless Debugging. Implement 500ms poll with 10-second timeout before surfacing an error.

---

## 9. References

- LADB: https://github.com/tytydraco/LADB (Apache 2.0) — Android-native ADB TLS + SPAKE2+ client
- Shizuku: https://github.com/RikkaApps/Shizuku (MIT) — server startup and IPC design
- dadb: https://github.com/mobile-dev-inc/dadb — JVM ADB client library
- Android ADB protocol source: https://android.googlesource.com/platform/packages/modules/adb/
- RFC 9382 — SPAKE2+
- KeyMapper (GitHub) — production shell-user key injection on Android
