# tap-mapper — implementation plan

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

## Epic 0 — Repository and toolchain setup

> Goal: a building, linting, and testing skeleton that all future work builds on.

- [x] **0.1** Initialise the Tauri 2.x project with `create-tauri-app`, selecting Svelte + TypeScript as the frontend framework
- [x] **0.2** Configure the Rust workspace with three crates: `mapping-core` (library), `tap-ble` (library), and `tap-tauri` (the Tauri binary)
- [x] **0.3** Add `mapping-core` as a path dependency of `tap-tauri` and `tap-ble`
- [x] **0.4** Configure `rustfmt` and `clippy` with a shared `rustfmt.toml` and `.clippy.toml` at workspace root
- [x] **0.5** Add ESLint, Prettier, and `svelte-check` to the frontend; wire them into a `pnpm lint` script
- [ ] **0.6** Set up a GitHub Actions CI pipeline: `cargo test`, `cargo clippy`, `cargo fmt --check`, `npm run lint`, `npm run check` on every pull request _(deferred — revisit at Epic 9 packaging)_
- [ ] **0.7** Add a `Makefile` or `justfile` with convenience targets: `dev`, `build`, `test`, `lint`, `check-all` _(deferred — revisit alongside 0.6 at Epic 9)_
- [x] **0.8** Create the `/profiles` directory convention and add a `.gitkeep` and a `README.md` explaining the expected directory structure
- [x] **0.9** Decide and document the per-OS profile storage path (e.g. `~/.config/mapxr/profiles/` on Linux, `%APPDATA%\mapxr\profiles\` on Windows, `~/Library/Application Support/mapxr/profiles/` on macOS) and implement a `platform::profile_dir()` helper in `src-tauri`

---

## Epic 1 — mapping-core data model

> Goal: all Rust types defined, serialising and deserialising correctly, with full test coverage.
> No BLE or UI involved — this epic is pure library work.

### 1a — Finger pattern type

- [x] **1.1** Define the `TapCode` newtype wrapping `u8`
- [x] **1.2** Implement `TapCode::fingers()` returning a struct with named boolean fields (`thumb`, `index`, `middle`, `ring`, `pinky`)
- [x] **1.3** Implement custom `Deserialize` for `TapCode` accepting:
  - 5-char string `"ooxoo"` with a `Hand` context (pinky-first or thumb-first)
  - 11-char dual string `"ooxoo ooxoo"` (space-separated, left pinky-first, right thumb-first)
  - Legacy `u8` integer
- [x] **1.4** Implement custom `Serialize` for `TapCode` always writing the string form (never the integer)
- [x] **1.5** Implement validation: reject `ooooo` as a standalone trigger code; reject patterns not exactly 5 or `5 5` chars; reject characters other than `o`/`x` (case-insensitive parse, lowercase canonical)
- [x] **1.6** Write unit tests covering all valid forms, all rejection cases, and round-trip serialisation
- [x] **1.16** Define `Hand` enum: `Left`, `Right` _(implemented early — required by TapCode parsing)_

### 1b — Trigger and action enums

- [x] **1.7** Define the `Trigger` enum: `Tap`, `DoubleTap`, `TripleTap`, `Sequence`
- [x] **1.8** Define the `TapStep` struct used inside `Sequence`
- [x] **1.9** Define the `Action` enum: `Key`, `KeyChord`, `TypeString`, `Macro`, `PushLayer`, `PopLayer`, `SwitchLayer`, `ToggleVariable`, `SetVariable`, `Block`, `Alias`
- [x] **1.10** Define `MacroStep` with an `action` field and a `delay_ms` field
- [x] **1.11** Define `KeyDef` as a validated string newtype; implement a `KeyDef::validate()` method that checks against the known key name list and returns a descriptive error for unknown names
- [x] **1.12** Define the `Modifier` enum: `Ctrl`, `Shift`, `Alt`, `Meta`
- [x] **1.13** Define the `PushLayerMode` enum: `Permanent`, `Count { count: u32 }`, `Timeout { timeout_ms: u64 }`
- [x] **1.14** Write unit tests for all enum variants, including serde round-trips and tag name validation

### 1c — Profile and settings structs

- [x] **1.15** Define `ProfileKind` enum: `Single`, `Dual`
- [x] **1.16** Define `Hand` enum: `Left`, `Right`
- [x] **1.17** Define `ProfileSettings` with all timing fields and `overload_strategy`
- [x] **1.18** Define `OverloadStrategy` enum: `Patient`, `Eager`
- [x] **1.19** Define `Mapping` struct: `label`, `trigger`, `action`, `enabled`
- [x] **1.20** Define `Profile` struct: all top-level fields including `version`, `kind`, `hand`, `layer_id`, `passthrough`, `settings`, `aliases`, `variables`, `on_enter`, `on_exit`, `mappings`
- [x] **1.21** Define `VariableValue` enum supporting `Bool(bool)` and `Int(i64)`
- [x] **1.22** Implement `Profile::load(path: &Path) -> Result<Profile, ProfileError>` with full validation on load:
  - Unknown key names in any `KeyDef`
  - Malformed finger patterns
  - Circular alias references
  - Overloaded codes without `overload_strategy` set
  - Macro nesting (a macro step may not contain another macro)
  - Dual pattern in a single-kind profile and vice versa
- [x] **1.23** Implement `Profile::save(path: &Path) -> Result<(), ProfileError>`
- [x] **1.24** Implement `ProfileError` with descriptive variants and human-readable `Display` messages
- [x] **1.25** Write unit tests for all validation rules; each rule should have at least one passing and one failing fixture

### 1d — Layer registry

- [x] **1.26** Define `LayerRegistry` that scans a directory, loads all valid `.json` files, and builds a `HashMap<LayerId, Profile>`
- [x] **1.27** Implement `LayerRegistry::reload()` to re-scan without restarting (for hot-reload support in the UI)
- [x] **1.28** Write tests using temp directories with fixture profile files

---

## Epic 2 — mapping-core engine

> Goal: the combo resolution, sequence detection, and action dispatch logic. Still no BLE or UI.
> The engine should be fully testable by pushing `TapEvent` structs in and observing `Action` outputs.

### 2a — Event types

- [x] **2.1** Define `RawTapEvent`: `device_id: DeviceId`, `tap_code: u8`, `received_at: Instant`
- [x] **2.2** Define `DeviceId` as a newtype over `String` (stable user-assigned role: `"left"`, `"right"`, or `"solo"`)
- [x] **2.3** Define `ResolvedEvent`: the output of the engine after combo/sequence resolution
- [x] **2.4** Define `EngineOutput`: either a `Vec<Action>` to execute or a `DebugEvent` (or both)

### 2b — Combo engine

- [x] **2.5** Implement `ComboEngine` struct with a pending event buffer and the active profile reference
- [x] **2.6** Implement `ComboEngine::push_event(event: RawTapEvent) -> Vec<EngineOutput>`: the main entry point
- [x] **2.7** Implement combo window logic: hold pending events for `combo_window_ms`; flush as singles on timeout
- [x] **2.8** Implement cross-device combo detection: match pending events from different devices within the window
- [x] **2.9** Implement overload detection at engine init: scan the active profile and build a set of overloaded codes
- [x] **2.10** Implement `patient` overload strategy: delay resolution of overloaded codes by `double_tap_window_ms`
- [x] **2.11** Implement `eager` overload strategy: fire immediately; queue an undo+replace if double-tap detected
- [x] **2.12** Implement double-tap detection
- [x] **2.13** Implement triple-tap detection
- [x] **2.14** Write unit tests for all timing scenarios: single tap, cross-device combo (within window), cross-device miss (outside window), double tap patient, double tap eager, triple tap

### 2c — Sequence engine

- [x] **2.15** Implement sequence progress tracking: maintain partial match state across events
- [x] **2.16** Implement per-step timeout: reset on each matched step, abort on timeout
- [x] **2.17** Implement per-trigger `window_ms` override
- [x] **2.18** Handle sequence abort: flush buffered steps as individual taps
- [x] **2.19** Write unit tests: full sequence match, step timeout, partial match then abort, sequence interleaved with non-matching tap

### 2d — Layer stack and dispatch

- [x] **2.20** Implement `LayerStack`: a `Vec<Profile>` with push/pop/switch operations
- [x] **2.21** Implement passthrough walk: on unmatched code, check `passthrough` flag and walk down the stack
- [x] **2.22** Implement `block` action stopping the passthrough walk
- [x] **2.23** Implement alias resolution: look up alias name in current layer's `aliases` map; resolve one level deep
- [x] **2.24** Implement variable state: per-layer `HashMap<String, VariableValue>`, initialised from profile on push
- [x] **2.25** Implement `toggle_variable` and `set_variable` dispatch
- [x] **2.26** Implement `push_layer` with all three modes (`permanent`, `count`, `timeout`)
- [x] **2.27** Implement count decrement: after each resolved trigger firing in a count-mode layer, decrement; pop on zero
- [x] **2.28** Implement timeout pop: use a tokio timer; fire `on_exit` and pop when elapsed
- [x] **2.29** Implement `pop_layer` with stack underflow guard
- [x] **2.30** Implement `on_enter` / `on_exit` action dispatch on push and pop
- [x] **2.31** Write unit tests for all layer operations: push/pop/switch, passthrough, block, count expiry, timeout expiry, variable toggle

### 2e — Debug event emission

- [x] **2.32** Define `DebugEvent` struct with all fields described in the spec (`resolved`, `unmatched`, `combo_timeout`)
- [x] **2.33** Implement debug event emission: when debug mode is enabled, attach a `DebugEvent` to every `EngineOutput`
- [x] **2.34** Implement debug mode toggle: `ComboEngine::set_debug(enabled: bool)`
- [x] **2.35** Write tests asserting that debug events contain correct timing metadata

---

## Epic 3 — BLE layer (`tap-ble`)

> Goal: discover, connect, and stream raw tap events from one or two Tap devices.
> Output is `RawTapEvent` structs consumed by the engine in Epic 2.

### 3a — Device discovery

- [x] **3.1** Add `btleplug` dependency to `tap-ble`
- [x] **3.2** Implement BLE adapter initialisation with error handling for adapter-not-found
- [x] **3.3** Implement device scan filtered by the Tap service UUID `C3FF0001-1D8B-40FD-A56F-C7BD5D0F3370`
- [x] **3.4** Implement scan result deduplication (same device appearing multiple times in scan)
- [x] **3.5** Expose `discover_devices() -> Vec<TapDeviceInfo>` returning name, address, and RSSI
- [x] **3.6** Write integration test (requires physical device or mock) that asserts the UUID filter works

### 3b — Connection and pairing

- [x] **3.\1** Implement `TapDevice::connect(address)` — connect and verify bond/pair status
- [x] **3.\1** Implement GATT service and characteristic discovery after connection; verify expected UUIDs are present
- [x] **3.\1** Implement graceful error if device is already connected to another host
- [x] **3.\1** Implement `TapDevice::disconnect()` ensuring the controller-mode exit packet is always sent before disconnect
- [x] **3.\1** Implement connection state change detection (device going out of range, battery death)
- [x] **3.\1** Implement automatic reconnect with exponential backoff on unexpected disconnect

### 3c — Controller mode

- [x] **3.\1** Implement `enter_controller_mode()`: write magic packet `[0x03, 0x0C, 0x00, 0x01]` to NUS RX characteristic `6E400002`
- [x] **3.\1** Implement the 10-second keepalive timer: re-send the enter packet every 10 seconds while connected
- [x] **3.\1** Implement `exit_controller_mode()`: write `[0x03, 0x0C, 0x00, 0x00]`
- [x] **3.16** Register a process exit hook (Tauri `on_window_event` for close) that calls `exit_controller_mode()` before shutdown
- [x] **3.\1** Write a test that asserts the exit packet is sent even when the app panics (use a drop guard)

### 3d — Tap data stream

- [x] **3.\1** Subscribe to notifications on the tap data characteristic `C3FF0005`
- [x] **3.\1** Implement packet parser: byte 0 = tap code `u8`, bytes 1–2 = interval `u16` little-endian
- [x] **3.\1** Emit `RawTapEvent` for each received notification, stamped with `Instant::now()` as `received_at`
- [x] **3.\1** Implement a `tokio::sync::broadcast` channel that distributes `RawTapEvent` to the engine
- [x] **3.\1** Write unit tests for the packet parser covering all valid tap codes (0–31) and edge cases (saturated interval 65535)

### 3e — Device registry

- [x] **3.23** Define `DeviceRegistry`: maps `DeviceId` (`"left"`, `"right"`, `"solo"`) to BLE hardware addresses
- [x] **3.24** Persist `DeviceRegistry` to a separate `devices.json` file in the config directory (not inside profile files)
- [x] **3.25** Implement `DeviceRegistry::assign(device_id, address)` and `DeviceRegistry::load/save`
- [x] **3.26** Implement role-validation at engine start: warn if a loaded profile requires roles not present in the registry

---

## Epic 4 — Tauri command layer (`tap-tauri`)

> Goal: bridge the Rust backend to the Svelte frontend via Tauri commands and events.

### 4a — Backend commands

- [x] **4.1** Implement `#[tauri::command] scan_devices() -> Vec<TapDeviceInfo>`
- [x] **4.2** Implement `#[tauri::command] connect_device(address: String, role: String) -> Result<(), String>`
- [x] **4.3** Implement `#[tauri::command] disconnect_device(role: String) -> Result<(), String>`
- [x] **4.4** Implement `#[tauri::command] list_profiles() -> Vec<ProfileSummary>`
- [x] **4.5** Implement `#[tauri::command] load_profile(layer_id: String) -> Result<Profile, String>`
- [x] **4.6** Implement `#[tauri::command] save_profile(profile: Profile) -> Result<(), String>`
- [x] **4.7** Implement `#[tauri::command] delete_profile(layer_id: String) -> Result<(), String>`
- [x] **4.8** Implement `#[tauri::command] activate_profile(layer_id: String) -> Result<(), String>`
- [x] **4.9** Implement `#[tauri::command] push_layer(layer_id: String, mode: PushLayerMode) -> Result<(), String>`
- [x] **4.10** Implement `#[tauri::command] pop_layer() -> Result<(), String>`
- [x] **4.11** Implement `#[tauri::command] set_debug_mode(enabled: bool)`
- [x] **4.12** Implement `#[tauri::command] get_engine_state() -> EngineStateSnapshot` (active layer stack, variable values, connected devices)

### 4b — Backend events (Rust → Svelte)

- [x] **4.13** Emit `tap-event` with `RawTapEvent` payload on every received tap (for live finger visualiser)
- [x] **4.14** Emit `action-fired` with action type and mapping label on every dispatched action
- [x] **4.15** Emit `layer-changed` with the new layer stack on every push/pop/switch
- [x] **4.16** Emit `device-connected` / `device-disconnected` with device info
- [x] **4.17** Emit `debug-event` with `DebugEvent` payload when debug mode is on
- [x] **4.18** Emit `profile-error` with a descriptive message if a profile fails to load

### 4c — State management

- [x] **4.19** Implement a `AppState` Tauri managed state struct holding: engine instance, BLE manager, device registry, layer registry
- [x] **4.20** Ensure all state access is properly guarded with `Mutex` or `RwLock`; document which locks are held during BLE callbacks to avoid deadlock
- [x] **4.21** Implement graceful shutdown: on app close, exit controller mode on all connected devices, flush any pending events

---

## Epic 5 — Svelte frontend — core

> Goal: a functional but unstyled UI covering all primary workflows.

### 5a — Tauri bindings and stores

- [x] **5.1** Generate TypeScript types from Rust command signatures using `tauri-specta` or write them manually; keep in a single `src/lib/types.ts`
- [x] **5.2** Create a `src/lib/commands.ts` wrapper module around all `invoke` calls with typed signatures
- [x] **5.3** Create a `src/lib/events.ts` module setting up all `listen` subscriptions with typed payloads
- [x] **5.4** Create a Svelte store `deviceStore`: connected devices, their roles and battery levels
- [x] **5.5** Create a Svelte store `engineStore`: active layer stack, variable values
- [x] **5.6** Create a Svelte store `profileStore`: list of available profiles, currently active profile
- [x] **5.7** Create a Svelte store `debugStore`: debug mode toggle, rolling buffer of last N debug events

### 5b — Device management page

- [x] **5.8** Implement device scan UI: scan button, list of discovered devices with name and signal strength
- [x] **5.9** Implement role assignment: connect a device and assign it `"solo"`, `"left"`, or `"right"`
- [x] **5.10** Implement connected device status panel: role, battery level, connection state indicator
- [x] **5.11** Implement disconnect button with confirmation
- [x] **5.12** Display a warning when a loaded profile's `required_roles` are not all connected

### 5c — Profile list page

- [x] **5.13** Implement profile list: name, kind badge (single/dual), description, active indicator
- [x] **5.14** Implement activate button: load a profile as the base layer
- [x] **5.15** Implement delete button with confirmation
- [x] **5.16** Implement new profile wizard: choose kind, name, hand (if single), create empty file
- [x] **5.17** Implement profile import: accept a `.json` file drop or file picker, validate, copy to profiles directory

### 5d — Profile editor

- [x] **5.18** Implement mapping list view: show all mappings with label, trigger summary, action summary, enabled toggle
- [x] **5.19** Implement mapping reorder (drag handles)
- [x] **5.20** Implement mapping enable/disable toggle (sets `enabled: false` in JSON without deleting)
- [x] **5.21** Implement add mapping button opening the mapping editor panel
- [x] **5.22** Implement delete mapping with undo (soft delete with 5-second undo toast)
- [x] **5.23** Implement the trigger editor panel:
  - [x] **5.23a** Tap type selector: tap / double_tap / triple_tap / sequence
  - [x] **5.23b** Finger pattern input widget (see Epic 6 for the visual component)
  - [x] **5.23c** Sequence step list: add/remove/reorder steps, per-step finger pattern input
  - [x] **5.23d** Per-trigger `window_ms` override field
- [x] **5.24** Implement the action editor panel:
  - [x] **5.24a** Action type selector dropdown
  - [x] **5.24b** `key`: key name input with autocomplete from valid key list, modifier checkboxes
  - [x] **5.24c** `key_chord`: multi-key input
  - [x] **5.24d** `type_string`: text area input
  - [x] **5.24e** `macro`: step list with action editor per step and delay_ms field
  - [x] **5.24f** `push_layer`: layer selector, mode selector, count/timeout fields conditionally shown
  - [x] **5.24g** `pop_layer` / `block`: no additional fields
  - [x] **5.24h** `toggle_variable`: variable name selector, on_true and on_false action editors
  - [x] **5.24i** `alias`: alias name selector from the profile's defined aliases
- [x] **5.25** Implement profile settings panel: all `settings` fields with labels and range hints
- [x] **5.26** Implement alias manager: list, add, edit, delete named aliases
- [x] **5.27** Implement variable manager: list, add, delete variables with type and initial value
- [x] **5.28** Implement `on_enter` / `on_exit` action editors at profile level
- [x] **5.29** Implement save button: write profile to disk, show success/error toast
- [x] **5.30** Implement unsaved changes guard: warn before navigating away with unsaved edits

---

## Epic 6 — Svelte frontend — finger pattern widget

> Goal: the custom visual component for entering and displaying finger patterns.
> Used in the trigger editor, live visualiser, and debug panel.

- [x] **6.1** Design the finger pattern component: 5 circles per hand (or 5 for single), filled/empty for x/o, clickable to toggle
- [x] **6.2** Implement `<FingerPattern>` Svelte component: accepts a `code` string prop, emits `change` events
- [x] **6.3** Implement hand label rendering: show T/I/M/R/P labels below each circle; account for read-direction (left hand labels reversed vs right)
- [x] **6.4** Implement dual-hand layout: two groups of 5 separated by a visual gap, left group labelled "Left", right labelled "Right"
- [x] **6.5** Implement read-only display mode (no click handlers): used in mapping list summaries and debug panel
- [x] **6.6** Implement keyboard input mode: allow typing `o`/`x` characters directly into the pattern when focused
- [x] **6.7** Implement "record" mode: when active, the next tap received from the hardware fills the pattern automatically (wires into `tap-event` listener)
- [x] **6.8** Validate the pattern on every change and show an inline error for invalid states (all-o, wrong length)
- [x] **6.9** Ensure the component renders correctly in both light and dark mode

---

## Epic 7 — Svelte frontend — live visualiser and debug panel

> Goal: real-time feedback showing what the devices are doing and why.

### 7a — Live finger visualiser

- [x] **7.1** Implement a persistent status bar or sidebar panel showing the current finger state of each connected device, updated live from `tap-event` emissions
- [x] **7.2** Show last-tap timestamp and tap code next to each device's finger display
- [x] **7.3** Show active layer stack as a breadcrumb (base > symbols > nav)
- [x] **7.4** Show current variable values for the active layer
- [x] **7.5** Animate finger circles briefly on tap (brief highlight that fades)

### 7b — Debug panel

- [x] **7.6** Implement debug mode toggle in the UI header; persists across sessions in local app config
- [x] **7.7** Implement the debug event stream: scrolling list of `DebugEvent` entries, newest at top
- [x] **7.8** Render `resolved` events: show finger pattern, matched layer, matched label, action fired, timing bars showing waited_ms vs window_ms
- [x] **7.9** Render `unmatched` events: show finger pattern, layers checked, reason
- [x] **7.10** Render `combo_timeout` events: show both pending patterns, combo window, actual gap
- [x] **7.11** Implement event type filter: checkboxes for resolved / unmatched / combo_timeout
- [x] **7.12** Implement pause/resume stream button
- [x] **7.13** Implement clear button
- [x] **7.14** Implement export: download the current debug stream as a `.jsonl` file for sharing bug reports

---

## Epic 8 — Profile normalisation CLI tool

> Goal: a command-line utility for working with profile files outside the GUI.

- [ ] **8.1** Add a `tap-cli` binary crate to the workspace
- [ ] **8.2** Implement `tap-mapper validate <file>`: load and validate a profile, print all errors with line references
- [ ] **8.3** Implement `tap-mapper normalize <file>`: rewrite all legacy integer `TapCode` values to finger-pattern strings, write back in place (with `--dry-run` flag)
- [ ] **8.4** Implement `tap-mapper migrate <file>`: apply all pending schema version migrations
- [ ] **8.5** Implement `tap-mapper lint <file>`: run validation plus optional warnings (e.g. overloaded codes without explicit strategy, very short combo windows)
- [ ] **8.6** Add `--output <path>` flag to `normalize` and `migrate` to write to a new file instead of in place
- [ ] **8.7** Write integration tests for each CLI command using fixture files

---

## Epic 9 — Packaging and distribution

> Goal: installable builds for Windows, macOS, and Linux.

- [ ] **9.1** Configure Tauri bundler for all three targets: `.msi` (Windows), `.dmg` (macOS), `.AppImage` + `.deb` (Linux)
- [ ] **9.2** Add application icons in all required sizes; design a simple icon (a stylised hand or tap symbol)
- [ ] **9.3** Configure auto-updater using Tauri's built-in updater pointing to GitHub Releases
- [ ] **9.4** Set up a GitHub Actions release workflow: trigger on version tag, build all targets, upload to GitHub Releases
- [ ] **9.5** Configure macOS code signing and notarisation (requires Apple Developer account)
- [ ] **9.6** Configure Windows code signing (optional but removes SmartScreen warnings)
- [ ] **9.7** Write a `CHANGELOG.md` and establish a versioning convention (semver: breaking profile schema changes = major)
- [ ] **9.8** Write a user-facing `README.md` covering installation, first-time setup, and a quick-start profile example

---

## Epic 10 — Android port

> Goal: the same UI and mapping logic running on Android via a Kotlin BLE bridge and WebView host.
> Intended as a later milestone; earlier epics are designed to make this straightforward.

- [ ] **10.1** Compile `mapping-core` to an Android-compatible `.so` using `cargo-ndk`; target `aarch64-linux-android` and `armv7-linux-androideabi`
- [ ] **10.2** Write a Kotlin `MappingCoreWrapper` JNI class exposing `pushEvent(tapCode: Int, deviceId: String): String` (returns JSON-encoded `Vec<Action>`)
- [ ] **10.3** Write a Kotlin BLE manager implementing the same controller-mode protocol as `tap-ble`
- [ ] **10.4** Set up an Android project with a `WebView` hosting the Svelte build from `src/`
- [ ] **10.5** Implement a `JavascriptInterface` bridge so the Svelte frontend can invoke Kotlin commands and receive events, mirroring the Tauri command/event API
- [ ] **10.6** Adapt the Svelte `commands.ts` and `events.ts` modules to detect the runtime (Tauri vs Android WebView) and route accordingly
- [ ] **10.7** Implement the Android BLE permissions flow (runtime permission requests for `BLUETOOTH_SCAN`, `BLUETOOTH_CONNECT`)
- [ ] **10.8** Test on a physical Android device; verify combo window timing is not degraded by WebView bridge latency
- [ ] **10.9** Publish to Google Play as a free app or provide an APK download on GitHub Releases

---

## Stretch goals (tracked but not scheduled)

> These are explicitly out of scope for the initial release. Listed here so they are not forgotten
> and so the schema/architecture decisions that accommodate them are visible.

- [ ] **S.1** Context-aware automatic layer switching: a daemon component monitoring active window / OS focus events, switching layers automatically. Lives outside `mapping-core`; uses the same `push_layer` / `pop_layer` API. Requires a separate `context-rules.json` schema.
- [ ] **S.2** Raw sensor / gesture triggers: tap-hold simulation via accelerometer duration, wrist rotation, air swipe. Requires subscribing to raw sensor mode notifications and building a gesture recognition pipeline on top of the 200Hz accelerometer stream.
- [ ] **S.3** iOS port: WKWebView host + Swift BLE bridge. Shares the same Svelte frontend. Lower priority than Android.
- [ ] **S.4** Community profile repository: a hosted index of shared profiles with an in-app browser and one-click import.
- [ ] **S.5** Plugin / scripting API: allow profiles to invoke a local HTTP endpoint or run a Lua script as an action, for integration with external tools (OBS, stream decks, home automation).

---

## Dependency map

```
Epic 0 (toolchain)
  └── Epic 1 (data model)
        └── Epic 2 (engine)
              └── Epic 3 (BLE)
                    └── Epic 4 (Tauri commands)
                          ├── Epic 5 (Svelte core UI)
                          │     ├── Epic 6 (finger widget)
                          │     └── Epic 7 (visualiser/debug)
                          └── Epic 8 (CLI tool)   ← depends on Epic 1+2 only

Epic 9 (packaging)   ← depends on Epics 0–7 being stable
Epic 10 (Android)    ← depends on Epic 1+2 (mapping-core) and Epic 5+6+7 (frontend)
```

Epics 6, 7, and 8 can all begin as soon as Epic 5a (stores and bindings) is complete.
Epic 8 (CLI) can begin as soon as Epic 1 is complete — it has no UI dependency at all.
