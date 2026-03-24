---
covers: Android native dispatch path — JNI bridge implementation (task 15.5 deviation fix)
status: Draft — awaiting approval
last-updated: 2026-03-21
---

# Android Background Dispatch — Specification

## Table of contents

1. [Problem statement](#1-problem-statement)
2. [Current implementation (what was built)](#2-current-implementation-what-was-built)
3. [Root cause](#3-root-cause)
4. [Target architecture](#4-target-architecture)
5. [Rust JNI layer](#5-rust-jni-layer)
6. [Kotlin integration](#6-kotlin-integration)
7. [Action dispatch path changes](#7-action-dispatch-path-changes)
8. [Double-dispatch elimination](#8-double-dispatch-elimination)
9. [Engine state sharing](#9-engine-state-sharing)
10. [UI events remain on the WebView path](#10-ui-events-remain-on-the-webview-path)
11. [Testing strategy](#11-testing-strategy)
12. [Files changed](#12-files-changed)

---

## 1. Problem statement

Mapped tap actions (key presses, text injection, mouse gestures) are silently dropped when
the MapXr app is in the background. The Tap device connects, BLE data arrives, the foreground
service stays alive — but nothing happens in the currently focused app.

**Expected behaviour:** A Tap action mapped to, e.g., `Space` should inject `Space` into
whatever Android app is in the foreground, regardless of whether MapXr is visible.

---

## 2. Current implementation (what was built)

The tap → action pipeline as actually implemented in `BlePlugin.kt` and `android-bridge.ts`:

```
Tap Strap (BLE)
  → onCharacteristicChanged() [Kotlin, native thread — always runs]
    → trigger("tap-bytes-received") [Tauri: Kotlin → WebView JS event]
      → addPluginListener callback [WebView JS — SUSPENDED in background]
        → invoke("process_tap_event") [WebView JS → Rust IPC]
          → ComboEngine.push_event() [Rust]
            → emit("tap-actions-fired") [Rust → WebView JS event]
              → listen callback [WebView JS — SUSPENDED in background]
                → invoke("dispatchActions") [WebView JS → Kotlin IPC]
                  → MapxrAccessibilityService.injectKey() [Kotlin — works fine]
```

Steps 3–7 run entirely through the WebView JS layer. Android's background process
management (Doze, App Standby, battery optimisation) throttles and eventually suspends
WebView JS execution when the app is not in the foreground. The BLE and Accessibility
layers run fine, but the signal never gets from one to the other.

### 2.1 What task 15.5 was supposed to build

`docs/spec/android-spec.md` §4.4 specifies a direct JNI bridge:

> On each characteristic notification, the Kotlin plugin receives a raw byte array. It passes
> this to the Rust `tap_packet_parse` JNI function exported from `mapping-core`.
> The Rust JNI function pushes the resulting `RawTapEvent` into the `ComboEngine` and returns
> a JSON-encoded `Vec<Action>` as a UTF-8 string. The Kotlin plugin deserialises the returned
> JSON and dispatches each action.

The current `BlePlugin.kt` implementation does not call any JNI function. It emits a WebView
event instead. Task 15.5 was marked complete but the JNI bridge was not implemented.

---

## 3. Root cause

The WebView path was used because it is simpler to implement (no JNI, no cross-language state
sharing) and works correctly in the foreground. The background failure was not caught during
testing because testing was done with the app open.

---

## 4. Target architecture

Replace the WebView-dependent middle section of the pipeline with a direct native path:

```
Tap Strap (BLE)
  → onCharacteristicChanged() [Kotlin, native thread — always runs]
    → NativeBridge.processTapBytes(address, bytes) [Kotlin → JNI → Rust]
      → ComboEngine.push_event() [Rust, shared engine instance]
        → Vec<Action> returned as JSON string [JNI return value]
          → AccessibilityDispatcher.dispatch(actions) [Kotlin — always runs]
            → MapxrAccessibilityService.injectKey() [Kotlin — always runs]
```

No WebView involvement in the action dispatch path. The signal travels from BLE callback to
key injection entirely in native code.

The WebView path is retained but repurposed: it receives processed events for UI display only
(device status, debug panel events, layer change notifications), not for action dispatch.

---

## 5. Rust JNI layer

### 5.1 What needs to be exported

A single JNI-callable function in the Tauri application crate
(`apps/desktop/src-tauri/src/`), not in `mapping-core`. It must reach the live `ComboEngine`
instance held in `AppState`.

```rust
// apps/desktop/src-tauri/src/android_jni.rs

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_com_mapxr_app_NativeBridge_processTapBytes(
    env: JNIEnv,
    _class: JClass,
    address: JString,
    bytes: JByteArray,
) -> jstring {
    // 1. Extract address and bytes from JNI types
    // 2. Get the shared AppState (via a static Arc — see §9)
    // 3. Parse bytes via the same packet parser used in tap-ble
    // 4. Push RawTapEvent into ComboEngine
    // 5. Collect returned Action(s)
    // 6. Serialise Vec<Action> to JSON
    // 7. Return as Java String (caller must not free)
}
```

The function name must follow the JNI naming convention exactly:
`Java_<package_underscored>_<class>_<method>`. For `com.mapxr.app.NativeBridge` calling
`processTapBytes`, this is `Java_com_mapxr_app_NativeBridge_processTapBytes`.

### 5.2 Dependencies

The `jni` crate must be added to `apps/desktop/src-tauri/Cargo.toml` under an Android
target guard:

```toml
[target.'cfg(target_os = "android")'.dependencies]
jni = "0.21"
```

`jni` is already used by the Android NDK build chain. It is well-maintained and compiles to
all Android ABI targets.

### 5.3 Packet parsing

The raw byte parsing logic currently lives in `crates/tap-ble/src/packet_parser.rs`. Two
options:

**Option A — re-export from `mapping-core`:** Add a `pub fn parse_tap_packet(bytes: &[u8]) -> Option<RawTapEvent>` in `crates/mapping-core/src/` that calls into the existing parser. This keeps all data types in one crate. `tap-ble` depends on `mapping-core`, so there is no circular dependency.

**Option B — call `tap-ble` from the JNI function:** The Tauri crate already depends on
`tap-ble` on desktop. On Android, `tap-ble` is excluded with `#[cfg(not(mobile))]`. Remove
that exclusion for the packet parser only, keeping the BLE connection code desktop-only.

**Recommended: Option A.** The packet parser is simple (5–10 lines) and belongs in
`mapping-core` rather than the BLE transport crate. Keep the JNI function independent of
`tap-ble`.

### 5.4 Return format

The function returns a JSON array of resolved actions, matching the existing `Action` serde
output used by the desktop pump:

```json
[
  { "type": "key", "key": "space", "modifiers": [] },
  { "type": "type_string", "text": "hello" }
]
```

On error (parse failure, lock poisoned, no engine state), return an empty JSON array `"[]"`
rather than null or throwing a JNI exception. Errors are logged via `android_log` or
`eprintln!` (logcat captures stderr on Android).

---

## 6. Kotlin integration

### 6.1 `NativeBridge` object

A new Kotlin object holds the `System.loadLibrary` call and the `external` declaration:

```kotlin
// NativeBridge.kt
package com.mapxr.app

object NativeBridge {
    init {
        // "mapxr_lib" matches the crate name in src-tauri/Cargo.toml ([lib] name = "mapxr_lib").
        // Confirmed: WryActivity.kt, Ipc.kt, and RustWebViewClient.kt all load this same library.
        System.loadLibrary("mapxr_lib")
    }

    external fun processTapBytes(address: String, bytes: ByteArray): String
}
```

The library is already loaded by the generated Tauri glue code (`WryActivity.kt`). Loading
it a second time from `NativeBridge` is safe — Android's class loader deduplicates library
loads. Alternatively, the `external fun` declaration can be placed directly on an existing
class that is already initialised after `WryActivity` runs, avoiding a redundant
`loadLibrary` call entirely.

### 6.2 Modify `BlePlugin.onTapBytes()`

Replace the `trigger("tap-bytes-received")` call with the native dispatch path:

```kotlin
private fun onTapBytes(address: String, bytes: ByteArray) {
    if (bytes.isEmpty()) return

    // Native path — processes the event through the Rust engine and dispatches actions
    // directly to the AccessibilityService. Works when the WebView is backgrounded.
    val actionsJson = NativeBridge.processTapBytes(address, bytes)
    AccessibilityDispatcher.dispatch(actionsJson)

    // WebView notification — for UI updates (debug panel, last-tap visualiser).
    // These events are best-effort: dropped silently if the WebView is suspended.
    val jsArray = JSArray()
    for (b in bytes) jsArray.put(b.toInt() and 0xFF)
    trigger("tap-bytes-received", JSObject().apply {
        put("address", address)
        put("bytes", jsArray)
    })
}
```

The WebView trigger is retained so the finger visualiser in the sidebar continues to work
when the app is in the foreground. `android-bridge.ts` listens for `tap-bytes-received` and
forwards it to the debug store for the sidebar visualiser — this listener must be kept but
must NOT call `dispatchActions` (see §8). UI events being dropped while backgrounded is
acceptable — the user cannot see the UI at that point.

Additionally, the Rust JNI function emits `tap-actions-fired` via the `AppHandle` (§9.1)
after processing each event. This keeps the debug panel populated with resolved events when
the user returns to the app. The `android-bridge.ts` listener for `tap-actions-fired` must
forward events to the debug store as before, but must NOT call `dispatchActions`.

---

## 7. Action dispatch path changes

### 7.1 Extract `AccessibilityDispatcher`

The action dispatch logic currently in `AccessibilityPlugin.dispatchActions()` (the
`when (type) { "key" -> ... }` block including `keyNameToCode` and `modifiersToMetaState`)
must be extracted into a standalone Kotlin object:

```kotlin
// AccessibilityDispatcher.kt
package com.mapxr.app

object AccessibilityDispatcher {
    fun dispatch(actionsJson: String) {
        val service = MapxrAccessibilityService.instance ?: run {
            Log.w(TAG, "dispatch: service not bound — actions dropped")
            return
        }
        val actions = try { JSONArray(actionsJson) } catch (_: Exception) { return }
        for (i in 0 until actions.length()) {
            val action = actions.optJSONObject(i) ?: continue
            dispatchOne(service, action)
        }
    }

    private fun dispatchOne(service: MapxrAccessibilityService, action: JSONObject) {
        when (val type = action.optString("type")) {
            "key"          -> { /* ... */ }
            "key_chord"    -> { /* ... */ }
            "type_string"  -> { /* ... */ }
            "mouse_click"  -> { /* ... */ }
            // etc.
        }
    }

    // keyNameToCode and modifiersToMetaState moved here from AccessibilityPlugin
}
```

`AccessibilityPlugin.dispatchActions()` becomes a thin wrapper:

```kotlin
@Command
fun dispatchActions(invoke: Invoke) {
    val data = invoke.getArgs() ?: run { invoke.resolve(); return }
    val actionsArray = try { data.getJSONArray("actions") } catch (_: Exception) {
        invoke.resolve(); return
    }
    AccessibilityDispatcher.dispatch(actionsArray.toString())
    invoke.resolve()
}
```

This keeps the WebView-callable command working for any future use, while the native path
also calls through the same `AccessibilityDispatcher`.

### 7.2 `LayerSwitch` and `ProfileSwitch` actions

These actions cannot be dispatched via the AccessibilityService — they update the engine's
layer stack. The existing Tauri pump (`pump.rs`) handles them reactively on desktop. The JNI
path must mirror this.

**Approach:** the JNI function calls the same post-action processing logic as `pump.rs`
after receiving the resolved action list:

1. For each `Action::LayerPush(id)` / `LayerPop` / `ProfileSwitch`: apply to the engine
   state directly (same code path as the Tauri command handlers).
2. Emit a `layer-changed` / `profile-changed` event to the WebView via the stored
   `AppHandle` (see §9.1). This keeps the UI in sync when the user returns to the app.
3. Do not include these action types in the JSON returned to Kotlin — they are fully
   consumed on the Rust side.

Non-state-mutating actions (`Key`, `TypeText`, `MouseClick`, `Vibrate`) are serialised
and returned to Kotlin for dispatch.

This means the JNI function does more than `push_event` alone. The full sequence is:
`parse_packet → push_event → process returned Vec<Action> → apply state actions → emit UI events → return dispatchable actions as JSON`.

---

## 8. Double-dispatch elimination

With the native path active, both paths could fire when the app is in the foreground:
the JNI path dispatches the action immediately, and then the WebView path (if still
functioning) would also dispatch it via `AccessibilityPlugin.dispatchActions()`.

**Resolution: remove `dispatchActions` from the `android-bridge.ts` `tap-actions-fired` listener.**

The `android-bridge.ts` listener for `tap-actions-fired` currently calls
`invoke("dispatchActions", ...)`. This call must be removed. The listener should only
forward the event payload to the debug store (for the debug panel and event log).

`tap-actions-fired` events are still emitted from the Rust JNI function via `AppHandle`
(§9.1) so the debug panel remains functional.

This makes the split clean and eliminates double-dispatch:
- **Native path (always):** BLE bytes → JNI → engine → Kotlin dispatch → AccessibilityService
- **WebView path (best-effort when foregrounded):** engine events → JS → debug store → debug panel UI

There is no double-dispatch risk because the WebView path no longer calls `dispatchActions`.

---

## 9. Engine state sharing

The `ComboEngine` is currently owned inside `AppState`, which is registered with Tauri via
`tauri::Builder::manage()`. The JNI function runs outside the Tauri command context and
cannot access `AppState` through the normal `tauri::State<T>` mechanism.

**Solution: static `Arc<AppState>`**

During app initialisation (`lib.rs`), store a clone of the `AppState` `Arc` in a module-level
static accessible to the JNI function:

```rust
// android_jni.rs
use std::sync::{Arc, OnceLock};

static ANDROID_STATE: OnceLock<Arc<AppState>> = OnceLock::new();

pub fn register_android_state(state: Arc<AppState>) {
    // OnceLock::set silently fails if already initialised (development hot-reload).
    // This is acceptable: on hot-reload the engine state persists and remains valid.
    let _ = ANDROID_STATE.set(state);
}
```

`build_app_state()` in `state.rs` returns an `Arc<AppState>`. In `lib.rs`, after building
app state, call `register_android_state(Arc::clone(&state))` before handing the `Arc` to
Tauri's `manage()`.

**Prerequisite:** `AppState` must use interior mutability (`Mutex` or `RwLock`) on its
`ComboEngine` field. **Verify this before starting the JNI work.** The existing Tauri
commands take `State<'_, AppState>` with `&self` (not `&mut self`), locking the inner mutex
per-call. If any field is behind `&mut self` without a lock, it must be converted.

**Thread safety note:** The JNI function is called from the Android BLE GATT callback thread
(`mainHandler` post). The Tauri command path uses the Tauri async executor thread. Both
acquire the same `Mutex<ComboEngine>` and serialise correctly. No special handling needed
beyond the existing lock.

### 9.1 App handle for event emission

`LayerSwitch`/`ProfileSwitch` actions need to emit Tauri events to the WebView (§7.2).
The `AppHandle` is available on the Tauri side; store a clone in `ANDROID_STATE` alongside
the `AppState`, or as a separate static, so the JNI function can call
`app_handle.emit("layer-changed", payload)`.

---

## 10. UI events remain on the WebView path

The following events continue to travel via the WebView path and require no changes:

| Event | Source | Consumer |
|-------|--------|----------|
| `ble-device-connected` | BlePlugin | deviceStore |
| `ble-device-disconnected` | BlePlugin | deviceStore |
| `ble-log` | BlePlugin | Devices page debug panel |
| `tap-bytes-received` | BlePlugin | android-bridge → debug store (finger visualiser) |
| `tap-actions-fired` | Rust engine | debug store (debug panel) |
| `layer-changed` | Rust engine | engineStore |
| `profile-changed` | Rust engine | profileStore |

These are informational. Losing them while backgrounded has no functional consequence —
the user cannot see the UI while the app is in background.

---

## 11. Testing strategy

### 11.1 Background dispatch test (manual)

1. Connect a Tap device; assign a role; load a profile with at least one mapped tap
2. Open a text input app (e.g. Google Keep) so it has focus
3. Minimise MapXr (press Home or switch to another app)
4. Tap the mapped finger combination on the Tap device
5. **Expected:** the mapped key action appears in the focused app
6. **Failure mode before fix:** nothing happens; after fix: key is injected

### 11.2 Foreground dispatch test (manual)

Same as 11.1 but with MapXr in the foreground. Verify:
- Key is injected once (not twice — no double-dispatch regression)
- Debug panel shows the tap event normally

### 11.3 Unit test — packet parser round-trip

In `crates/mapping-core/src/tap_parser.rs` (new file per §5.3, Option A):

```rust
#[test]
fn parse_tap_packet_single_finger_parses_correctly() { ... }

#[test]
fn parse_tap_packet_empty_bytes_returns_none() { ... }
```

### 11.4 Unit test — JNI return format

A Rust unit test (no JNI machinery needed) that calls the inner action-serialisation
function and verifies the output is valid JSON matching the expected schema.

---

## 12. Files changed

| File | Change |
|------|--------|
| `apps/desktop/src-tauri/src/android_jni.rs` | New — JNI export, static state registration |
| `apps/desktop/src-tauri/src/lib.rs` | Call `register_android_state` during startup (Android only) |
| `apps/desktop/src-tauri/src/state.rs` | Ensure `AppState` is `Arc`-wrapped and fields use interior mutability |
| `apps/desktop/src-tauri/Cargo.toml` | Add `jni = "0.21"` under Android target |
| `crates/mapping-core/src/tap_parser.rs` | New — public `parse_tap_packet` function |
| `gen/android/.../NativeBridge.kt` | New — `System.loadLibrary` + `external fun` declaration |
| `gen/android/.../AccessibilityDispatcher.kt` | New — extracted from `AccessibilityPlugin` |
| `gen/android/.../BlePlugin.kt` | Modify `onTapBytes`: call `NativeBridge`, retain WebView trigger for UI only |
| `gen/android/.../AccessibilityPlugin.kt` | Thin wrapper delegating to `AccessibilityDispatcher` |
| `apps/desktop/src/lib/android-bridge.ts` | Remove `dispatchActions` invoke from `tap-actions-fired` listener |
