# Architectural decisions

Non-obvious design choices made during the project. Consult this document before changing
any of the affected areas. Add a new entry whenever a significant decision is made —
especially ones where the reason would not be obvious from reading the code alone.

---

## Profile persistence is handled in Rust, not the frontend

Storing the last-active profile in `localStorage` would cause a brief flicker of the wrong
profile during app startup, because the Svelte app loads after the Tauri backend. By
persisting to `preferences.json` on the Rust side and loading it in `build_app_state()`,
the correct profile is active in the engine before the first IPC message is sent.

**Affected files:** `apps/desktop/src-tauri/src/state.rs`

---

## `DeviceId` is a `String` label, not an enum

Device roles ("left", "right", "solo") are user-assigned identifiers, not hardware-fixed
identities. Using a `String` newtype means new roles can be introduced without a schema
change, and it matches the way profile JSON files reference devices. An enum would require
a code change every time a new role was needed and would complicate the Android port.

**Affected files:** `crates/mapping-core/src/types/` — `DeviceId`

---

## `mapping-core` has no async dependency

The core library is pure synchronous logic. This keeps it portable (including the eventual
Android port via JNI), fully testable without a Tokio runtime, and free from the complexity
of async cancellation. Timing-sensitive operations (combo windows, sequence timeouts) are
driven by `Instant` values passed in by the caller rather than internal timers.

**Affected files:** `crates/mapping-core/` — entire crate

---

## Eager overload strategy was removed rather than fixed

The eager strategy fires a tap action immediately and undoes it if a double-tap is later
detected. This undo mechanism is intractable for the general action vocabulary (chords,
macros, layer ops, variable changes). The patient strategy — wait for the double-tap window
before resolving — is correct in all cases and is now automatic whenever a code has both
`tap` and `double_tap`/`triple_tap` bindings. No `overload_strategy` field is needed in
profiles.

**Affected files:** `crates/mapping-core/src/engine/combo_engine.rs`

---

## BLE reconnect max backoff is 10 s

The original cap was 60 s, which is past the threshold where users assume the software is
broken and start toggling things. 10 s is the practical UX ceiling for a user who has just
powered on a device and expects it to reconnect promptly. The backoff sequence is
1 s → 2 s → 4 s → 8 s → 10 s → 10 s indefinitely.

**Affected files:** `crates/tap-ble/src/tap_device.rs` — `RECONNECT_MAX_DELAY`

---

## `pop_layer` returns `Option<Vec<EngineOutput>>`, not `Vec<EngineOutput>`

An empty `Vec` was ambiguous: both "stack underflow (already at base layer)" and "successful
pop with no `on_exit` actions" returned `vec![]`. The Tauri command misread a clean pop with
no `on_exit` as an underflow error and failed to emit `layer-changed`, causing UI/engine
desync. `None` now means underflow; `Some(vec![])` means a clean pop with no output.

**Affected files:** `crates/mapping-core/src/engine/combo_engine.rs` — `pop_layer()`,
`apps/desktop/src-tauri/src/commands.rs`, `apps/desktop/src-tauri/src/pump.rs`

---

## The app uses a frameless window with a custom Svelte title bar

On KDE Plasma (Wayland), hiding and re-showing a window with server-side decorations
causes KWin to lose track of its decoration state. The title bar buttons (close, minimize,
maximize) become visually present but unresponsive to hover and click until some compositor
event forces a re-evaluation (e.g. double-clicking the title bar to maximize). No amount of
`set_focus()`, `set_always_on_top()` toggling, or timing delays fixes this — it is a
fundamental limitation of how KWin manages server-side decorations for windows that
disappear and reappear via `hide()`/`show()`.

The solution used by Electron apps (Discord, VS Code, Spotify) is `decorations: false` plus
a custom HTML title bar. With no compositor-managed frame, there is nothing for KWin to lose
track of. `win.hide()` and `win.show()` work correctly and the custom buttons are ordinary
Svelte elements unaffected by compositor state.

**Affected files:** `apps/desktop/src-tauri/tauri.conf.json` — `"decorations": false`,
`apps/desktop/src/lib/components/TitleBar.svelte`,
`apps/desktop/src/routes/+layout.svelte`

---

## Haptic BLE payload must always be exactly 20 bytes

The Tap device firmware requires the haptic characteristic write (`C3FF0009`) to be exactly
20 bytes: a 2-byte header (`[0x00, 0x02]`) followed by 18 duration slots. If a shorter payload
is written, the firmware reads uninitialised RAM for the remaining slots and plays phantom
on/off durations — producing additional unexpected buzzes after the intended pattern.

The fix is to always zero-pad the payload to 20 bytes regardless of how many durations are in
the pattern, matching the behaviour of the C# SDK. The Python SDK example in
`docs/reference/vibration.txt` shows short payloads but that happens to work because the Python
SDK's underlying BLE library pads writes to the characteristic's declared value length.

**Affected files:** `crates/mapping-core/src/types/action.rs` — `VibrationPattern::encode()`

---

## Device role reassignment reuses the existing BLE connection

When a user reassigns a device from one role to another (e.g. "solo" → "right"),
`DeviceId` is just a label stamped onto events. The underlying BLE connection, GATT
characteristics, controller mode, and notification subscription are all properties of the
`Peripheral` object and are unaffected. The only work needed is cancelling and restarting
the three background tasks (keepalive, notification reader, connection monitor) under the
new `DeviceId`.

**Affected files:** `crates/tap-ble/src/tap_device.rs` — `TapDevice::reassign()`,
`crates/tap-ble/src/manager.rs` — `BleManager::reassign_role()`
