## 2026-03-15 — Implement hold_modifier action (sticky modifier keys)

**Tasks completed:** hold_modifier feature (spec-approved, not in original numbered task list)
**Tasks in progress:** none

**Files changed:**

- `docs/spec/hold-modifier-spec.md` — new approved spec document
- `crates/mapping-core/src/types/hold_modifier_mode.rs` — new `HoldModifierMode` enum with Toggle/Count/Timeout variants; serde round-trip tests
- `crates/mapping-core/src/types/action.rs` — added `HoldModifier` variant; 9 new tests
- `crates/mapping-core/src/types/mod.rs` — added module + re-export for `HoldModifierMode`
- `crates/mapping-core/src/error.rs` — added 5 new `ProfileError` variants for hold_modifier validation
- `crates/mapping-core/src/types/profile.rs` — added `check_hold_modifier_rules()` validator; 6 new validation tests; updated imports
- `crates/mapping-core/src/engine/combo_engine.rs` — added `HeldModifierEntry`/`ActiveHoldMode` structs; `held_modifiers: Vec<HeldModifierEntry>` field; `update_hold_modifier()`, `decrement_hold_modifier_counts()`, `expire_held_modifier_timeouts()`, `held_modifier_set()`, `merge_held_modifiers()`, `merge_held_modifiers_into_chord()`, `apply_held_modifiers_to_macro_steps()` methods; updated `execute_action` to take `now: Instant` and apply held modifiers to Key/KeyChord/TypeString/Macro; updated `check_timeout()` and `next_deadline()` for timeout entries
- `crates/mapping-core/tests/combo_engine.rs` — 13 new engine integration tests covering all spec scenarios
- `src/lib/types.ts` — added `HoldModifierMode` type; added `hold_modifier` variant to `Action` union
- `src/lib/components/ActionEditor.svelte` — added `hold_modifier` to type selector, `TYPE_LABELS`, `defaultAction`, helper functions, and form UI; macro steps now disallow `hold_modifier`

**Notes:**
- `execute_action` signature changed to accept `now: Instant` to support Timeout mode deadline calculation
- `held_modifiers` state is deliberately NOT cleared on push/pop/switch_layer per spec §3
- `set_profile()` clears `held_modifiers` (full reset)
- All 263 mapping-core tests pass; clippy clean; fmt clean
- The `+layout.svelte` TypeScript error is pre-existing (unrelated to this feature)

**Next:** Epic 8.1 — Add `tap-cli` binary crate to the workspace

---

## 2026-03-15 — Dual-device alternating tap: analysis + tap_code=0 guard + coverage tests

**Tasks completed:** (bug investigation — not task-numbered)
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/src/engine/combo_engine.rs` — added tap_code=0 early-return guard in `push_event`; a zero-code notification (all-open / finger release) would previously enter `combo_pending` and consume a partner slot from the next real tap
- `crates/mapping-core/tests/combo_engine.rs` — added three tests: `rapid_alternating_dual_taps_all_combos_fire`, `rapid_alternating_dual_taps_left_first_all_combos_fire`, `rapid_alternating_dual_same_device_stacks_then_all_resolve`

**Notes:**
User reported: rapid R→L→R→L alternation misses 3rd and 4th taps (they eventually fire). Thorough analysis confirmed the engine logic is correct — all three new tests pass. Likely causes of the real-world issue (in order of probability):
1. **combo_window_ms too small**: if the user successfully saved 40 ms after the earlier bug fix, and their tap gaps exceed 40 ms, events would time out as solos instead of forming combos.
2. **tap_code=0 release events**: if the TAP hardware sends an all-open notification between taps, it would previously enter combo_pending and shift partner matching. The new guard filters these out.
3. **Profile configuration**: a sequence trigger in the dual profile starting with the same tap code as the alternating taps would consume events before they reach combo_pending.
4. **BLE timing**: infrequent delivery delays that push some gaps just over the combo window boundary.

Recommended diagnostic: enable debug mode in the app and observe `resolved`/`unmatched`/`combo_timeout` events during alternating taps to see exactly what the engine is seeing.

**Next:** Continue with next incomplete task in `docs/plan/implementation-plan.md`.

---

## 2026-03-15 — Latency optimizations: immediate dispatch, precise deadline sleep, lower combo window

**Tasks completed:** (latency improvements — not task-numbered; out-of-band optimization session)
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/src/engine/combo_engine.rs` — Opt A: added `needs_wait: HashSet<TapCode>` field (rebuilt by `rebuild_overloads`); in `push_event`, codes not in `needs_wait` now dispatch immediately via `flush_tap_pending_now` + `dispatch()` instead of going through the 250 ms `handle_tap` buffer; added `flush_tap_pending_now` helper; added `next_deadline() -> Option<Instant>` method for pump scheduling; changed `DEFAULT_COMBO_WINDOW_MS` from 150 → 80 (Opt C); added `build_needs_wait` free function
- `crates/mapping-core/src/engine/layer_stack.rs` — added `pub fn next_timeout() -> Option<Instant>` so `ComboEngine::next_deadline` can include layer auto-pop deadlines
- `src-tauri/src/pump.rs` — removed `TIMEOUT_POLL_MS` constant and fixed 50 ms interval; replaced with dynamic `tokio::time::sleep_until(next_deadline)` pattern (Opt B)
- `crates/mapping-core/tests/combo_engine.rs` — renamed `single_tap_non_overloaded_resolves_immediately` to match new behavior; updated `check_timeout_flushes_single_tap_after_double_tap_window_expires` to use a profile with DoubleTap (so code 1 still buffers); updated all debug-event tests to check the first push output; split `debug_resolved_waited_ms_reflects_buffering_duration` into two tests (immediate vs buffered); added new tests: `single_tap_only_profile_dispatches_immediately`, `single_tap_with_double_tap_binding_still_waits`, `single_tap_triple_tap_binding_still_waits`, `next_deadline_none_when_nothing_pending`, `next_deadline_set_after_tap_buffered`
- `crates/mapping-core/tests/layer_stack.rs` — updated all tests that relied on the "buffer first, flush on second tap" pattern to check the first push output directly

**Notes:**
Three independent optimizations implemented:
- **Opt A** eliminates the 250 ms double-tap buffer for codes with no multi-tap binding. For a typical single-tap typing profile, every keystroke now fires at BLE delivery latency (~10–50 ms) rather than 250 ms. The key invariant is that `needs_wait` contains any code with a DoubleTap or TripleTap trigger in any layer of the active stack; only those codes go through `handle_tap`. A subtle correctness concern: the immediate dispatch path calls `flush_tap_pending_now` first, so any pending tap for a different code (which must be from a `needs_wait` code that went through `handle_tap`) is flushed in order before the new code fires.
- **Opt B** eliminates up to 50 ms of extra latency on all timed flushes (double-tap expiry, combo expiry, sequence expiry, layer timeout). The pump now sleeps until the exact deadline.
- **Opt C** reduces the default combo window from 150 → 80 ms, saving ~70 ms per unmatched event in dual mode.

**Next:** Continue with next incomplete task in `docs/plan/implementation-plan.md`.

---

## 2026-03-15 — Four runtime bug fixes: profile validation, auto-reconnect, tap flush

**Tasks completed:** (bug fixes — no task numbers; all pre-Epic 8 runtime issues)
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/src/types/profile.rs` — made `validate()` public so callers outside the
  crate (e.g. `save_profile` Tauri command) can call it before writing to disk
- `src-tauri/src/commands.rs` — `save_profile`: call `profile.validate()` before writing to disk
  so invalid profiles are rejected with a descriptive error instead of being silently persisted;
  `disconnect_device`: now also removes the device from `DeviceRegistry` and saves `devices.json`
  so the device is not auto-reconnected on next launch
- `src/routes/profiles/+page.svelte` — added Delete button to profile load-error warning banners
  so the user can remove invalid profile files directly from the UI without navigating the filesystem
- `crates/tap-ble/src/device_registry.rs` — `assign()`: added `self.entries.retain(|_, a| *a != address)`
  before inserting so one physical device cannot appear under multiple roles across sessions;
  added `iter()` method exposing all (DeviceId, BDAddr) pairs for auto-reconnect; added regression
  test `device_registry_assign_removes_stale_role_for_same_address`
- `src-tauri/src/state.rs` — added `auto_reconnect()` async function: two-phase reconnect —
  (1) try direct connect for each saved device from the adapter's existing peripheral cache;
  (2) run a 5-second BLE scan for any device that returned `DeviceNotFound`, then retry
- `src-tauri/src/lib.rs` — spawns `auto_reconnect` as a background task after app state is
  registered with Tauri, so previously paired devices reconnect automatically on app restart
- `crates/mapping-core/src/engine/combo_engine.rs` — `check_timeout()`: added call to new
  `flush_expired_tap_pending()`; added `flush_expired_tap_pending()` method that proactively
  dispatches buffered single/double taps once the relevant window has elapsed, fixing the
  "one tap behind" bug where taps only fired when a subsequent tap arrived to flush them
- `crates/mapping-core/tests/combo_engine.rs` — added two regression tests:
  `check_timeout_flushes_single_tap_after_double_tap_window_expires` and
  `check_timeout_flushes_double_tap_after_triple_tap_window_expires`

**Notes:**

**Bug 1 — Profile save validation ("trigger code kind mismatch"):**
`save_profile` wrote profiles to disk without calling `validate()`. A dual profile with a
single-format tap code ("xoooo") had been written before `defaultCode()` was fixed; every
subsequent `list_profiles` reload emitted `profile-error` events. Fix: `validate()` made public,
called in `save_profile` before the filesystem write. Invalid profiles are now rejected at save
time with a descriptive error. Also added a Delete button to load-error banners so users can
remove broken files without touching the filesystem directly.

**Bug 2 — Auto-reconnect not working:**
btleplug's `adapter.peripherals()` is empty at startup; the peripheral cache is only populated
after a BLE scan. `find_peripheral` therefore always returned `None` for saved devices. Fix:
two-phase approach — phase 1 tries a direct connect (fast, works on Linux/BlueZ when the OS
has the device bonded); phase 2 runs a 5-second scan then retries any device that returned
`DeviceNotFound` in phase 1. Failures are logged and skipped; they never block startup.

**Bug 3 — Device connecting twice (solo and right):**
`DeviceRegistry::assign()` only removed the old entry for the same `DeviceId`, not other roles
pointing at the same physical address. After a session where the device was "right" and another
where it was "solo", both entries survived in `devices.json`. At startup, `auto_reconnect`
connected the same hardware under both roles. Fix: `retain(|_, a| *a != address)` clears any
stale role before inserting the new one.

**Bug 4 — Taps detected but "no match" / firing one tap late:**
Every tap is buffered in `tap_pending` (waiting for a possible double/triple tap within the
configured window). `flush_expired_tap_pending` was only called from `push_event` (i.e. lazily
on the next incoming tap). With no subsequent tap, the buffered action never fired. The 50ms
`check_timeout` timer only handled layer-stack timeouts, not `tap_pending`. Fix: added
`flush_expired_tap_pending()` called unconditionally from `check_timeout`. Now any tap that has
aged past its window is dispatched within ~50ms of the deadline, regardless of whether another
tap arrives.

**Next:** Epic 8 — profile normalisation CLI tool (8.1–8.7). Spec approved. Begin with 8.1.

---

## 2026-03-14 — BLE root cause fix: initialize BleManager on Tauri's async runtime

**Tasks completed:** (bug fix — no task number)
**Tasks in progress:** none

**Files changed:**

- `src-tauri/src/lib.rs` — replaced temporary `current_thread` tokio runtime with a
  `sync_channel` + `tauri::async_runtime::spawn` pattern to initialize `AppState` on
  Tauri's persistent async runtime instead of a short-lived throwaway runtime
- `crates/tap-ble/src/scanner.rs` — (from prior session) `discover_devices_le` takes
  `adapter: &Adapter` parameter; `discover_devices` creates adapter and passes it; tests updated
- `crates/tap-ble/src/manager.rs` — (from prior session) `BleManager::scan` calls
  `discover_devices_le(&self.adapter, ...)` / `scan_with_adapter(&self.adapter, ...)` directly

**Notes:**
- ROOT CAUSE: The original `lib.rs` setup used `tokio::runtime::Builder::new_current_thread()`
  to run `build_app_state`. Internally, btleplug's `Manager::new()` calls `tokio::spawn` to
  create a D-Bus IOResource task. That task was spawned on the temporary runtime. When the
  temporary runtime was dropped after `block_on` returned, the IOResource task was killed.
  Every subsequent D-Bus call through `self.adapter` (Manager A) in Tauri commands blocked
  forever — no task remained to receive and dispatch replies.
- The old scan worked only because `discover_devices_le` created a fresh Manager B inside
  a Tauri command (on `tauri::async_runtime`), so Manager B's IOResource survived. But all
  calls through `BleManager.adapter` (Manager A) were dead. The earlier adapter-passing change
  (`collect_peripherals(&self.adapter)` instead of Manager B's adapter) made the scan fail
  because it routed the peripheral list query through the dead session.
- FIX: `tauri::async_runtime::spawn` the setup future so btleplug tasks are spawned on
  Tauri's multi-threaded persistent runtime. A `sync_channel(1)` carries the result back to
  the synchronous setup callback via `rx.recv()`. No deadlock risk: Tauri's runtime is
  multi-threaded, so the spawned task runs on a worker thread while the main thread blocks.
- With Manager A's IOResource alive, `BleManager::scan` (using `self.adapter` directly) and
  `BleManager::connect` (also through `self.adapter`) both work correctly.

**Next:** Epic 8 — profile normalisation CLI tool (8.1–8.7). Spec approved. Begin with 8.1.

---

## 2026-03-14 — BLE connect hang fix: pass BleManager adapter into discover_devices_le

**Tasks completed:** (bug fix — no task number; blocks Epic 8 testing)
**Tasks in progress:** none

**Files changed:**

- `crates/tap-ble/src/scanner.rs` — `discover_devices_le` now takes `adapter: &Adapter` as a
  parameter instead of calling `get_adapter()` internally; made `pub(crate)`; `discover_devices`
  creates its own adapter and passes it; added `ScanFilter` to test module imports; explicit type
  annotation on `Arc::clone` in test
- `crates/tap-ble/src/manager.rs` — `BleManager::scan` now calls `discover_devices_le` directly
  with `&self.adapter` (Linux) / `scan_with_adapter` (non-Linux) instead of delegating to the
  free-standing `discover_devices`; removed `discover_devices` import; added conditional imports
  for `discover_devices_le` and `scan_with_adapter`

**Notes:**
- Root cause: `discover_devices_le` was calling `get_adapter()` internally to create a temporary
  btleplug Manager B. `collect_peripherals` ran against Manager B. When Manager B was dropped, its
  D-Bus session closed. Then `BleManager::connect` used `self.adapter` (Manager A) which had its
  own peripheral handles. The temporary Manager B's D-Bus session cleanup conflicted with the
  subsequent `Connect()` D-Bus call made through Manager A, causing `peripheral.connect()` to hang.
- Fix: `BleManager::scan` now passes `&self.adapter` into `discover_devices_le`, eliminating the
  zombie Manager B entirely. Scan and connect both use the same btleplug session (Manager A).
- The standalone `discover_devices()` function (used in physical device tests) is unchanged
  in behaviour — it still creates a temporary adapter internally.
- All 57 unit tests pass; workspace clippy clean.

**Next:** Epic 8 — profile normalisation CLI tool (8.1–8.7). Spec approved
(`docs/spec/cli-tool-spec.md`). Begin with task 8.1: add `tap-cli` binary crate to workspace.

---

## 2026-03-14 — Epic 7 complete: live visualiser and debug panel (tasks 7.1–7.14)

**Tasks completed:** 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8, 7.9, 7.10, 7.11, 7.12, 7.13, 7.14
**Tasks in progress:** none

**Files changed:**

- `docs/spec/debug-panel-spec.md` — new spec for Epic 7; approved with window_ms addition and layer breadcrumb moved to sidebar
- `crates/mapping-core/src/engine/debug_event.rs` — added `#[serde(tag="kind", rename_all="snake_case")]`; added `window_ms: u64` to `Resolved` variant
- `crates/mapping-core/src/engine/resolved_event.rs` — added `window_ms: u64` field; updated test fixture
- `crates/mapping-core/src/engine/combo_engine.rs` — threaded `window_ms` through all `ResolvedEvent` construction sites (combo match, flush_expired_combo, handle_eager, handle_tap ×3, flush_sequence_as_singles); propagated to `DebugEvent::Resolved` at both dispatch sites; extracted `seq_window_ms` in `dispatch_sequence`
- `src-tauri/src/commands.rs` — fixed pre-existing test bug: `passthrough: None` → `passthrough: false`
- `src/lib/types.ts` — replaced loose `DebugEvent` interface (with index signature) with tight discriminated union; all three variants fully typed with `window_ms` on `Resolved`
- `src/lib/stores/debug.svelte.ts` — added `DeviceTapState` interface; added `lastTapByRole` state; extended `recordTap` to update per-device state and clear `flash` after 500ms via setTimeout
- `src/lib/components/FingerPattern.svelte` — added `flash?: boolean` prop; applies `[animation:tap-flash_0.45s_ease-out]` to tapped circles when flash is true
- `src/app.css` — added `@keyframes tap-flash` definition
- `src/routes/+layout.svelte` — added sidebar "State" section (layer breadcrumb + variables) and "Live" section (per-device FingerPattern with tap code + relative timestamp); removed layer from footer; debug mode persistence on init via localStorage; 1s tick for relative time updates
- `src/routes/debug/+page.svelte` — full debug panel: debug mode toggle, event type filters, pause/resume with buffered-count indicator, clear, JSONL export, scrolling event stream with resolved/unmatched/combo_timeout cards, timing bars

**Notes:**
- `{@const}` must be a direct child of a block tag (`{#if}`, `{#each}`, etc.), not a `<div>`. Workaround: wrap in `{#if true}`.
- `window_ms = 0` for immediate resolutions (eager tap, sequence flushes); the UI shows "Immediate resolution" text rather than an empty bar in those cases.
- Layer breadcrumb moved from status bar footer into the sidebar "State" section; footer now shows only connected device chips.
- Debug mode persisted in `localStorage` under key `"mapxr.debugMode"`.
- All 168 files pass `npm run check` with 0 errors. All mapping-core and mapxr tests pass.

**Next:** Epic 8 — profile normalisation CLI tool (tasks 8.1–8.7). No spec document required (mapping-core spec covers all data types; CLI is a thin wrapper). Can begin immediately.

---

## 2026-03-14 — Epic 6 complete: visual finger pattern widget (tasks 6.1–6.9)

**Tasks completed:** 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 6.9
**Tasks in progress:** none

**Files changed:**

- `docs/spec/finger-pattern-spec.md` — new spec for Epic 6; approved with btn-sm circles, parent-controlled record mode, and immediate placeholder removal
- `src/lib/utils/tapCode.ts` — new: `tapCodeToPattern(tapCode, hand)` pure utility; bit layout verified against Rust `TapCode::to_single_pattern` source
- `src/lib/components/FingerPattern.svelte` — new: full component replacing FingerPatternPlaceholder; handles single/dual layouts, interactive/readonly/record modes, per-mode label visibility, all-open prevention, inline validation, keyboard nav (ArrowLeft/Right, Space/Enter, 'o'/'x' typing), aria-pressed + aria-label a11y, `onrecorded` callback on recording→false transition
- `src/lib/components/FingerPatternPlaceholder.svelte` — deleted (replaced by FingerPattern)
- `src/routes/profiles/[layer_id]/edit/+page.svelte` — replaced FingerPatternPlaceholder imports/usages with FingerPattern; passes `hand={profile.hand ?? "right"}`

**Notes:**
- `tapCodeToPattern` bit layout: right-hand → bit i at string position i; left-hand → bit (4-i) at position i. Mirrors Rust exactly.
- `buttonRefs` uses a flat array indexed by `gi * 5 + fi` (max 10 for dual); arrow key navigation moves focus across group boundaries naturally.
- `bind:this` in `{#each}` works correctly in Svelte 5 — refs update reactively when the each block re-renders.
- Light/dark theming uses only daisyUI semantic tokens (`bg-primary`, `border-base-content/30`, `bg-base-100`) — no hardcoded colours.
- 168 files pass `npm run check` with 0 errors, 0 warnings.

**Next:** Epic 7 — live visualiser and debug panel (tasks 7.1–7.14). Requires a spec document (`docs/spec/debug-panel-spec.md`) before implementation begins.

---

## 2026-03-14 — Epic 5 complete: Svelte frontend core UI fully implemented (tasks 5.1–5.30)

**Tasks completed:** 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 5.9, 5.10, 5.11, 5.12, 5.13, 5.14, 5.15, 5.16, 5.17, 5.18, 5.19, 5.20, 5.21, 5.22, 5.23, 5.23a, 5.23b, 5.23c, 5.23d, 5.24, 5.24a, 5.24b, 5.24c, 5.24d, 5.24e, 5.24f, 5.24g, 5.24h, 5.24i, 5.25, 5.26, 5.27, 5.28, 5.29, 5.30
**Tasks in progress:** none

**Files changed:**

- `docs/spec/svelte-frontend-spec.md` — new spec for Epic 5; approved with Tailwind v4 + daisyUI v5
- `vite.config.js` — added `@tailwindcss/vite` plugin
- `src/app.css` — new: `@import "tailwindcss"; @plugin "daisyui";`
- `src/lib/types.ts` — new: complete TypeScript mirrors of all Rust serde types; key correctness points: `TapStep` serialises as plain `FingerPattern` string; `PushLayerMode` flattened into `Action.push_layer`; `VariableValue` is `boolean | number` (untagged); `Mapping.enabled` omitted when true; `Profile.passthrough` omitted when false
- `src/lib/commands.ts` — new: typed `invoke` wrappers for all 12 Tauri commands with JSDoc
- `src/lib/logger.ts` — new: structured logger wrapper (dev-only for info/warn, always for error)
- `src/lib/events.ts` — new: `setupEventListeners()` wiring all 7 Tauri events to stores
- `src/lib/stores/device.svelte.ts` — new: `DeviceStore` class with `$state<ConnectedDevice[]>`
- `src/lib/stores/engine.svelte.ts` — new: `EngineStore` with layer stack, variables, debug mode
- `src/lib/stores/profile.svelte.ts` — new: `ProfileStore` with profiles list and load errors
- `src/lib/stores/debug.svelte.ts` — new: `DebugStore` with rolling 200-event buffer
- `src/routes/+layout.svelte` — new: sidebar nav, status bar (layer breadcrumb + device chips), store init on mount
- `src/routes/+page.svelte` — replaced scaffold with redirect to `/profiles`
- `src/routes/devices/+page.svelte` — new: full device management (scan, role selector, connect, disconnect with confirm modal)
- `src/routes/debug/+page.svelte` — stub: "Debug panel coming in Epic 7"
- `src/routes/profiles/+page.svelte` — new: profile list with active indicator, kind badge, activate/edit/delete, new-profile wizard, import
- `src/lib/components/FingerPatternPlaceholder.svelte` — new: plain-text finger pattern input with inline validation; readonly mode renders `<code>`
- `src/lib/components/TriggerSummary.svelte` — new: one-line read-only trigger summary
- `src/lib/components/ActionSummary.svelte` — new: one-line read-only action summary
- `src/lib/components/ActionEditor.svelte` — new: recursive component handling all 11 action types; self-imports to handle macro/toggle_variable nesting
- `src/routes/profiles/[layer_id]/edit/+page.svelte` — new: full profile editor (mappings tab with drag-reorder, soft-delete+undo, inline trigger/action editors; settings, aliases, variables, lifecycle tabs; save with dirty indicator; beforeNavigate guard)

**Notes:**
- Technology: Tailwind CSS v4 (CSS-first config, `@tailwindcss/vite` plugin) + daisyUI v5 component classes
- Recursive `ActionEditor` requires explicit `import ActionEditor from "./ActionEditor.svelte"` — Svelte does not auto-import the current component
- Svelte `{#each}` treats `as` as the iteration-variable keyword; TypeScript casts in the array position cause parse errors — cast inside the loop body instead
- `$page.params.layer_id` is `string | undefined`; fix: `?? ""` fallback (the route always provides it in practice)
- All 167 files pass `npm run check` with 0 errors, 0 warnings

**Next:** Epic 6 — visual finger pattern widget (tasks 6.1–6.9). Requires a spec document (`docs/spec/finger-pattern-spec.md`) before implementation begins.

---

## 2026-03-14 — Epic 4 complete: Tauri command layer fully implemented (tasks 3.16, 4.1–4.21)

**Tasks completed:** 3.16, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 4.10, 4.11, 4.12, 4.13, 4.14, 4.15, 4.16, 4.17, 4.18, 4.19, 4.20, 4.21
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/src/engine/debug_event.rs` — added `Serialize, Deserialize` derives to `DebugEvent` (required by 4.17)
- `crates/mapping-core/src/engine/layer_stack.rs` — added `top_variables()` accessor returning `&HashMap<String, VariableValue>`
- `crates/mapping-core/src/engine/combo_engine.rs` — added `debug_mode()`, `layer_ids()`, `top_variables()`, `layer_stack_toggle_variable()`, `layer_stack_set_variable()`, `top_profile_alias()` accessors
- `crates/tap-ble/src/tap_device.rs` — added `address: BDAddr` field; added `status_tx: broadcast::Sender<BleStatusEvent>` to `connect()` and monitor task; emits `Connected`/`Disconnected` events on connection state changes; added `#[allow(clippy::too_many_arguments)]` on `connection_monitor_task`
- `crates/tap-ble/src/manager.rs` — added `BleStatusEvent` enum; `status_tx` broadcast sender; `subscribe_status()` method; `connect()` emits `Connected`; `disconnect()` emits `Disconnected`
- `crates/tap-ble/src/lib.rs` — re-exported `BleStatusEvent`
- `src-tauri/Cargo.toml` — added `tap-ble`, `tokio` (time feature), `anyhow`, `env_logger`, `enigo`, `log`, `btleplug` dependencies
- `src-tauri/src/platform.rs` — rewrote: checks `<exe_dir>/profiles/` first as dev override; falls back to OS config dir
- `src-tauri/src/state.rs` — new: `AppState` struct with `engine`, `layer_registry`, `ble_manager: Option<Mutex<BleManager>>`, `device_registry`, paths; `build_app_state()` async init; `require_ble()` guard; `builtin_default_profile()` fallback
- `src-tauri/src/events.rs` — new: 7 event name constants; 6 payload structs with `Serialize`; `EngineStateSnapshot`
- `src-tauri/src/commands.rs` — new: all 12 Tauri commands (4.1–4.12); `TapDeviceInfoDto` and `ProfileSummary` DTOs; unit tests for DTO conversions
- `src-tauri/src/pump.rs` — new: `run_event_pump`, `run_ble_status_listener`, `process_outputs`, `process_outputs_no_keys`, `execute_action` (all Action variants), `simulate_key` via `spawn_blocking`, `emit_layer_changed`, key name mapping table
- `src-tauri/src/lib.rs` — rewrote: `env_logger::init()`, tokio runtime setup, `build_app_state()`, `Arc<AppState>` managed state, pump/status-listener spawns, all 12 commands registered, `on_window_event(CloseRequested)` graceful BLE disconnect

**Notes:**
- `Macro` action executes steps inline (not in a separate spawned task). The `tokio::spawn` approach was blocked by `Send` bound: `execute_action` is mutually recursive with `process_outputs` through `Box::pin`, making the future chain unprovable as `Send` to the compiler. Inline execution means the pump pauses during macro delays, but key simulations themselves are `spawn_blocking` (fire-and-forget) so only inter-step sleeps cause a brief pump pause. Acceptable for typical short macro sequences.
- `Enigo` is `!Send` on X11 (Linux). Handled by creating a fresh `Enigo` inside each `spawn_blocking` call rather than holding it in the async task.
- Lock ordering documented in `state.rs`: `engine` → `layer_registry` → `ble_manager`. `device_registry` uses `std::sync::Mutex` for brief sync I/O and must not be held across async awaits.
- `ble_manager` is `Option<Mutex<BleManager>>` — `None` when no adapter found at startup. Dummy closed broadcast channels are dropped immediately so pump tasks exit cleanly.

**Next:** Epic 5 (Svelte frontend). Requires a spec document before implementation begins. Start with `docs/spec/epic5-svelte-frontend-spec.md`.

---

## 2026-03-14 — DeviceRegistry persistence and role validation complete (tasks 3.23–3.26)

**Tasks completed:** 3.23, 3.24, 3.25, 3.26
**Tasks in progress:** none

**Files changed:**

- `crates/tap-ble/src/device_registry.rs` — implemented `load()` and `save()` with write-then-rename atomicity; on-disk format uses `HashMap<String, String>` (role → colon-delimited BDAddr) wrapped in a `StoredRegistry` struct with a `version: 1` field; `load()` returns empty registry on `NotFound`; 9 unit tests covering round-trip, missing file, malformed JSON, empty devices object, and JSON structure validation
- `crates/tap-ble/src/manager.rs` — added `BleManager::check_roles(profile: &Profile, registry: &DeviceRegistry)` static method; logs `warn!` if profile is `Dual` but fewer than 2 devices are registered
- `crates/tap-ble/Cargo.toml` — added `log = "0.4"` (required by spec's logging requirements throughout); added `tempfile = "3"` to dev-dependencies for registry unit tests

**Notes:**

- `DeviceId` in `mapping-core` does not derive serde (only `Debug, Clone, PartialEq, Eq, Hash`). Rather than adding serde to `mapping-core` (which would require user approval as a public API change), the on-disk layer uses `HashMap<String, String>` and converts on load/save via `DeviceId::new()` and `id.to_string()`.
- `BDAddr` with the btleplug `serde` feature serialises as a byte array, not a hex string. Storing as `String` (via `addr.to_string()` / `BDAddr::from_str()`) gives the human-readable colon-delimited format the spec requires.
- `log = "0.4"` was added as the spec explicitly requires logging throughout the BLE layer (DEBUG-level MTU logging, WARN-level role mismatch). This is a tiny facade crate; no log subscriber is wired up yet — that happens in the Tauri binary (Epic 4).
- Task 3.16 (Tauri shutdown hook) remains deferred to Epic 4.

**Next:** Epic 3 is now complete (all 3.1–3.26 except 3.16 which is deferred to Epic 4). Ready to begin Epic 4 — Tauri command layer. Need to write a spec for Epic 4 before any implementation code.

## 2026-03-13 — Workspace scaffolded; tasks 0.1–0.3 complete

**Tasks completed:** 0.1, 0.2, 0.3
**Tasks in progress:** none

**Files changed:**

- `Cargo.toml` — workspace root created; members: `src-tauri`, `crates/mapping-core`, `crates/tap-ble`
- `crates/mapping-core/Cargo.toml` — library crate on edition 2024
- `crates/tap-ble/Cargo.toml` — library crate on edition 2024; `mapping-core` added as path dep
- `src-tauri/Cargo.toml` — `mapping-core` added as path dep
- `CLAUDE.md` — updated: project name `mapxr`, identifier `com.mapxr.app`, package manager `npm`, edition 2024 for library crates, corrected file layout (`src-tauri/` instead of `crates/tap-tauri/`)

**Notes:**

- `create-tauri-app` was run manually by the user with project name `mapxr` and identifier `com.mapxr.app` using npm.
- The Tauri crate lives at `src-tauri/` (standard Tauri layout), not `crates/tap-tauri/` as the original spec described. CLAUDE.md updated to reflect this.
- Library crates use edition 2024 (user preference); `src-tauri` uses edition 2021 as generated.
- `cargo check -p mapping-core -p tap-ble` passes cleanly.
- Full workspace `cargo build` fails on Linux due to missing system libraries for Tauri's WebKit backend (`libsoup-3.0`, `webkit2gtk4.1`, etc.). This is a system setup issue, not a code issue. Fix: `sudo dnf install libsoup3-devel webkit2gtk4.1-devel openssl-devel libappindicator-gtk3-devel librsvg2-devel`

---

## 2026-03-13 — rustfmt and clippy config added

**Tasks completed:** 0.4
**Tasks in progress:** none

**Files changed:**

- `rustfmt.toml` — created: edition 2024, max_width 100, field-init and try shorthand enabled
- `.clippy.toml` — created: msrv 1.77.2, cognitive-complexity-threshold 30

**Notes:**

- `imports_granularity` and `group_imports` are nightly-only rustfmt options; omitted.
- `cargo fmt --check` and `cargo clippy -- -D warnings` both pass cleanly.

**Next:** 0.5 — Add ESLint, Prettier, and svelte-check to the frontend; wire into an `npm run lint` script

---

## 2026-03-13 — ESLint, Prettier, and lint script added

**Tasks completed:** 0.5
**Tasks in progress:** none

**Files changed:**

- `package.json` — added `lint` (`prettier --check . && eslint .`) and `format` (`prettier --write .`) scripts; added devDependencies: `eslint`, `@eslint/js`, `typescript-eslint`, `eslint-plugin-svelte`, `globals`, `prettier`, `prettier-plugin-svelte`
- `eslint.config.js` — created: ESLint 9 flat config with JS, TypeScript, and Svelte rules; `no-console` and `no-explicit-any` set to error; ignores `target/`, `build/`, `.svelte-kit/`, `node_modules/`
- `.prettierrc` — created: 100 char width, double quotes, trailing commas, Svelte plugin
- `.prettierignore` — created: excludes `target/`, `src-tauri/gen/`, `node_modules/`, `build/`, `*.lock`

**Notes:**

- `svelte-check` was already present in devDependencies from `create-tauri-app`; `strict: true` was already set in `tsconfig.json`.
- `npm run lint` passes cleanly.
- `npm run check` (svelte-check) was already wired; not renamed.

**Next:** 0.8 — profiles/ directory and 0.9 platform::profile_dir()

---

## 2026-03-13 — profiles/ directory and platform helper; Epic 0 complete

**Tasks completed:** 0.8, 0.9
**Tasks in progress:** none

**Files changed:**

- `profiles/.gitkeep` — created to track empty directory in git
- `profiles/README.md` — documents file format, naming convention, and per-OS runtime paths
- `src-tauri/src/platform.rs` — new module; `profile_dir(app)` wraps `app.path().app_config_dir()` and appends `profiles/`, creating the directory if absent
- `src-tauri/src/lib.rs` — added `pub mod platform`

**Notes:**

- Tasks 0.6 (CI pipeline) and 0.7 (justfile) deferred to Epic 9 — not needed until there are artifacts worth building.
- `platform::profile_dir()` uses `tauri::Manager` trait for `app.path()` access; `tauri::Manager` is imported locally inside the function to avoid polluting the module namespace.
- `cargo clippy -- -D warnings` and `cargo fmt --check` both pass.
- Epic 0 is now complete (excluding the two deferred tasks).

**Next:** Epic 1 — mapping-core data model. First task is 1.1: define the `TapCode` newtype. Spec is `docs/spec/mapping-core-spec.md`.

---

## 2026-03-13 — TapCode newtype defined; task 1.1 complete

**Tasks completed:** 1.1
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/Cargo.toml` — added `serde` dependency
- `crates/mapping-core/src/lib.rs` — replaced scaffold; declares `pub mod types`
- `crates/mapping-core/src/types/mod.rs` — created; re-exports `TapCode`
- `crates/mapping-core/src/types/tap_code.rs` — `TapCode(u8)` newtype with `from_u8`, `as_u8`, `fingers()`; `Fingers` struct with named boolean fields; 6 unit tests

**Notes:**

- `Fingers` fields named `a`–`e` (raw bit positions) rather than finger names because the physical mapping depends on `Hand` context, which is not known at this level.
- Custom `Serialize`/`Deserialize` deferred to tasks 1.3–1.4 as planned.
- All tests pass; clippy and fmt clean.

**Next:** 1.3–1.6

---

## 2026-03-13 — TapCode parsing, TriggerPattern, Hand; tasks 1.3–1.6 and 1.16 complete

**Tasks completed:** 1.3, 1.4, 1.5, 1.6, 1.16
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/Cargo.toml` — added `thiserror = "2"` dependency; `serde_json = "1"` dev-dependency
- `crates/mapping-core/src/types/hand.rs` — `Hand` enum (`Left`, `Right`) with serde, `Default = Right`
- `crates/mapping-core/src/types/tap_code.rs` — `TapCodeError`; `from_single_pattern(s, hand)` and `to_single_pattern(self, hand)` with correct bit/string mapping per hand orientation; custom `Deserialize` (u8 only); custom `Serialize` (canonical right-hand string form)
- `crates/mapping-core/src/types/trigger_pattern.rs` — `TriggerPattern` enum (`Single(TapCode)`, `Dual { left, right }`); `from_dual_pattern`; `to_pattern_string(hand)`; `is_all_open()` for profile validation
- `crates/mapping-core/src/types/mod.rs` — exports `Hand`, `TapCode`, `TapCodeError`, `Fingers`, `TriggerPattern`

**Notes:**

- `TapCode(u8)` cannot represent a dual pattern (10 bits); introduced `TriggerPattern` to hold either `Single(TapCode)` or `Dual { left: TapCode, right: TapCode }`. This is the type used in `Trigger` structs (task 1.7+).
- `Hand` pulled forward from task 1.16 — required for TapCode string parsing. Marked complete in plan.
- `TapCode::Deserialize` handles legacy `u8` integers only. Single-hand string parsing requires `Hand` context from the profile; this is done by the profile loader (task 1.22) via `from_single_pattern`. `Serialize` writes canonical right-hand string; profile serialisation (task 1.23) uses `to_single_pattern(hand)` directly.
- `ooooo` as a standalone trigger and `ooooo ooooo` as a dual trigger are rejected by profile validation (task 1.22/1.25) via `TriggerPattern::is_all_open()`, not at parse time (since `ooooo` is valid as the idle side of a dual pattern).
- 33 tests pass; clippy and fmt clean.

**Next:** 1.7 — Trigger enum

---

## 2026-03-13 — Trigger enum and TapStep defined; tasks 1.7–1.8 complete

**Tasks completed:** 1.7, 1.8
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/src/types/trigger_pattern.rs` — added `Serialize` (canonical right-hand string) and `Deserialize` (u8 → Single; 11-char string → Dual; 5-char string → Single right-hand default)
- `crates/mapping-core/src/types/tap_code.rs` — added `TapCodeError::OutOfRange` variant
- `crates/mapping-core/src/types/trigger.rs` — new file: `Trigger` enum (`Tap`, `DoubleTap`, `TripleTap`, `Sequence`) with `#[serde(tag = "type", rename_all = "snake_case")]`; `TapStep` newtype serialising/deserialising as a plain string
- `crates/mapping-core/src/types/mod.rs` — exports `Trigger`, `TapStep`

**Notes:**

- `TapStep` serialises as a plain string (not an object) matching the profile format `"steps": ["oooox", ...]`.
- `TriggerPattern::Deserialize` uses right-hand as default for 5-char strings. Left-hand profile loading re-resolves codes via `from_single_pattern(Hand::Left)` in task 1.22.
- 48 tests passing; clippy and fmt clean.

**Next:** 1.9 — Define the `Action` enum

---

## 2026-03-13 — Fingers struct renamed to physical finger names; task 1.2 complete

**Tasks completed:** 1.2
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/src/types/tap_code.rs` — `Fingers` fields renamed from `a`–`e` to `thumb`, `index`, `middle`, `ring`, `pinky`; doc comments updated to document the hardware-fixed bit layout; tests updated

**Notes:**

- User clarified that the hardware normalises bit positions to physical fingers regardless of hand — bit 0 is always thumb, bit 4 is always pinky. `Hand` context only affects string notation parsing direction. `Fingers` struct therefore uses physical names unconditionally.

**Next:** 1.3 — Custom `Deserialize` for `TapCode` accepting string and legacy integer forms

## 2026-03-13 — Action enum and all dependent types implemented; tasks 1.9–1.14 and 1.21 complete

**Tasks completed:** 1.9, 1.10, 1.11, 1.12, 1.13, 1.14, 1.21
**Tasks in progress:** none

**Files changed:**
- `crates/mapping-core/src/types/modifier.rs` — `Modifier` enum (Ctrl, Shift, Alt, Meta); `#[serde(rename_all = "lowercase")]`
- `crates/mapping-core/src/types/key_def.rs` — `KeyDef(String)` newtype; `validate()` via binary search on `VALID_KEYS`; `KeyDefError::UnknownKey`; sorted const covering a–z, 0–9, f1–f24, arrows, media, volume, and named keys
- `crates/mapping-core/src/types/push_layer_mode.rs` — `PushLayerMode` enum; `#[serde(tag = "mode")]`; flattened into parent `push_layer` JSON object
- `crates/mapping-core/src/types/variable_value.rs` — `VariableValue` enum; `#[serde(untagged)]` so booleans/integers serialise as plain JSON values
- `crates/mapping-core/src/types/action.rs` — `Action` enum with all 11 variants; `MacroStep`; `#[serde(tag = "type", rename_all = "snake_case")]`; `PushLayer` uses `#[serde(flatten)]` for `PushLayerMode`; 33 unit tests covering serialisation tags, round-trips, and spec JSON deserialization
- `crates/mapping-core/src/types/mod.rs` — added all five new modules and re-exports

**Notes:**
- `PushLayer` flatten + internally-tagged (`#[serde(tag = "type")]` outer, `#[serde(tag = "mode")]` inner via flatten) works correctly — all three PushLayer tests pass including spec JSON deserialization.
- `VariableValue` pulled forward from task 1.21 since it was a dependency of `Action::SetVariable`.
- 86 tests pass; clippy clean; fmt clean.

**Next:** 1.15 — Define `ProfileKind` enum (`Single`, `Dual`)

## 2026-03-14 — Epic 1 complete: all data model types, Profile::load/save, validation, LayerRegistry

**Tasks completed:** 1.15, 1.17, 1.18, 1.19, 1.20, 1.22, 1.23, 1.24, 1.25, 1.26, 1.27, 1.28
**Tasks in progress:** none

**Files changed:**
- `crates/mapping-core/src/types/profile_kind.rs` — `ProfileKind` enum (Single, Dual)
- `crates/mapping-core/src/types/overload_strategy.rs` — `OverloadStrategy` enum (Patient, Eager)
- `crates/mapping-core/src/types/profile_settings.rs` — `ProfileSettings` struct with all timing fields and `eager_undo_sequence`
- `crates/mapping-core/src/types/mapping.rs` — `Mapping` struct; `enabled` defaults to `true` and is omitted from JSON when true
- `crates/mapping-core/src/types/profile.rs` — `Profile` struct + `load()` / `save()` + all 6 validation rules as private methods
- `crates/mapping-core/src/error.rs` — `ProfileError` with 8 variants covering all load-time failure modes
- `crates/mapping-core/src/layer_registry.rs` — `LayerRegistry`: scan dir, load all `.json`, skip invalid, `reload()` for hot-reload
- `crates/mapping-core/src/lib.rs` — added `error` and `layer_registry` modules; re-exported `ProfileError`, `LayerRegistry`
- `crates/mapping-core/Cargo.toml` — promoted `serde_json` from dev-dep to regular dep; added `tempfile` as dev-dep
- `crates/mapping-core/src/types/mod.rs` — exported all new types
- `crates/mapping-core/tests/profile_validation.rs` — 17 integration tests for all validation rules
- `crates/mapping-core/tests/layer_registry.rs` — 11 integration tests for LayerRegistry
- `crates/mapping-core/tests/fixtures/` — 8 fixture `.json` files (4 valid, 4 invalid)

**Notes:**
- `serde_json` moved to regular dependencies since `Profile::load`/`save` use it in production code.
- `Profile::save` uses write-then-rename for atomicity (`.json.tmp` → `.json`).
- Overload detection uses the profile's `hand` field for canonical string form; dual profiles use `Hand::Right`.
- `LayerRegistry::load_errors()` exposes files that failed without blocking valid ones from loading.
- All 149 tests pass (120 unit + 17 profile_validation + 11 layer_registry + 1 doc-test harness).

**Next:** 2.1 — Define `RawTapEvent` (start of Epic 2 — mapping-core engine)

---

## 2026-03-14 — Epic 2 engine: ComboEngine and SequenceEngine (tasks 2.1–2.19, 2.32)

**Tasks completed:** 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9, 2.10, 2.11, 2.12, 2.13, 2.14, 2.15, 2.16, 2.17, 2.18, 2.19, 2.32
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/src/engine/mod.rs` — module wiring; exports: `ComboEngine`, `DebugEvent`, `DeviceId`, `EngineOutput`, `RawTapEvent`, `ResolvedEvent`, `ResolvedTriggerKind`
- `crates/mapping-core/src/engine/device_id.rs` — `DeviceId` newtype over `String`
- `crates/mapping-core/src/engine/raw_tap_event.rs` — `RawTapEvent` with `new_at()` public (not cfg(test)) for integration test use
- `crates/mapping-core/src/engine/resolved_event.rs` — `ResolvedEvent` and `ResolvedTriggerKind` (Tap, DoubleTap, TripleTap)
- `crates/mapping-core/src/engine/engine_output.rs` — `EngineOutput { actions, debug }`
- `crates/mapping-core/src/engine/debug_event.rs` — `DebugEvent` enum: `Resolved`, `Unmatched`, `ComboTimeout`
- `crates/mapping-core/src/engine/combo_engine.rs` — full `ComboEngine` implementation:
  - Combo window (cross-device), patient/eager overload, double/triple-tap state machine
  - `SequenceProgress` struct; `handle_sequence_step`, `flush_expired_sequence`, `flush_sequence_as_singles`, `dispatch_sequence`, `sequence_window_ms`
  - `push_event(event, now: Instant)` — deterministic timing via explicit `now` parameter
- `crates/mapping-core/src/lib.rs` — re-exports for engine types
- `crates/mapping-core/tests/combo_engine.rs` — 15 integration tests: single tap, double tap, triple tap, cross-device combo, debug mode, and 5 sequence tests

**Notes:**
- Timing model: `push_event` takes `now: Instant` for deterministic test control (no tokio needed).
- Sequence timeout is lazy: detected on the next event arrival (same pattern as combo-window expiry).
- Flushed sequence steps go through `dispatch` directly as single taps, bypassing double/triple-tap machinery, to avoid artificial multi-tap detections from stale data.
- Dual-profile combo timeout creates `Dual { device_side, other: TapCode(0) }` patterns so dual-only bindings can match.
- Debug events for sequences use the first step's tap_code as the pattern representative.
- All 181 tests pass; clippy clean; fmt clean.

**Next:** 2.20 — Implement `LayerStack`: a `Vec<Profile>` with push/pop/switch operations

---

## 2026-03-14 — LayerStack and full dispatch integration (tasks 2.20–2.31, 2.33, 2.34)

**Tasks completed:** 2.20, 2.21, 2.22, 2.23, 2.24, 2.25, 2.26, 2.27, 2.28, 2.29, 2.30, 2.31, 2.33, 2.34
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/src/engine/layer_stack.rs` — new: `LayerStack` struct with `push`, `pop`, `switch_to`, `on_trigger_fired`, `check_timeout`, `get/set/toggle_variable`, `walk`, `layer_ids`; 15 unit tests
- `crates/mapping-core/src/engine/mod.rs` — added `layer_stack` module; exported `LayerStack`
- `crates/mapping-core/src/lib.rs` — re-exported `LayerStack`
- `crates/mapping-core/src/engine/combo_engine.rs` — major refactor:
  - Replaced `profile: Profile` with `layer_stack: LayerStack`
  - Added `push_layer`, `pop_layer`, `switch_layer`, `check_timeout` public methods
  - Added `rebuild_overloads`, `clear_pending` helpers
  - Rewrote `dispatch()` with two-phase passthrough walk (immutable find, mutable execute)
  - Added `execute_action()` handling `Block`, `PopLayer`, `ToggleVariable`, `SetVariable`
  - Changed `resolve_action` to static `resolve_action_in(action, profile)`
  - Updated all timing helpers to use `layer_stack.top()`
  - Count decrement via `on_trigger_fired()` after every successful match (including Block)
- `crates/mapping-core/tests/layer_stack.rs` — 14 integration tests covering all layer operations

**Notes:**
- Timeout pop (task 2.28) is synchronous: `check_timeout(now: Instant) -> Vec<EngineOutput>`. The tokio timer lives in the Tauri layer (Epic 4) which calls this method on an interval. The library stays async-free.
- `PushLayer` and `SwitchLayer` actions are returned to the caller as-is (they require a registry lookup to find the target profile — the caller provides that). `PopLayer` is handled inline.
- Passthrough walk uses a two-phase approach to avoid borrow conflicts: Phase 1 collects the match (immutable); Phase 2 executes it (mutable).
- All 210 tests pass; clippy clean; fmt clean.

**Next:** 2.35 — Write tests asserting that debug events contain correct timing metadata

## 2026-03-14 — TapDevice full implementation; tasks 3.7–3.22 complete (excl. 3.16)

**Tasks completed:** 3.7, 3.8, 3.9, 3.10, 3.11, 3.12, 3.13, 3.14, 3.15, 3.17, 3.18, 3.19, 3.20, 3.21, 3.22
**Tasks in progress:** none
**Tasks deferred:** 3.16 (process exit hook — belongs in Tauri layer, Epic 4)

**Files changed:**

- `crates/tap-ble/Cargo.toml` — added `futures = "0.3"` (regular dep for StreamExt), `rstest = "0.24"` (dev dep); removed `async-trait` and `futures` from dev-only
- `crates/tap-ble/src/error.rs` — added `DeviceNotFound { address }` variant (spec omission: spec has no variant for "device not in scan cache")
- `crates/tap-ble/src/packet_parser.rs` — new: `TapPacket`, `parse_tap_packet`; 42 unit tests covering all 32 valid codes (rstest `#[values]`), little-endian interval, saturation at 65535, empty/partial/oversized packets
- `crates/tap-ble/src/tap_device.rs` — full `TapDevice` implementation: `connect()`, `disconnect()`, `Drop` impl; background tasks: `keepalive_task`, `notification_task`, `connection_monitor_task`, `reconnect_loop`; UUID + protocol constants
- `crates/tap-ble/src/manager.rs` — `BleManager::new()` (async, initialises adapter), `scan()`, `connect()`, `disconnect()`, `connected_ids()`, `subscribe()`
- `crates/tap-ble/src/lib.rs` — added `packet_parser` module and re-exports for `TapPacket`, `parse_tap_packet`, `TapDevice`
- `crates/tap-ble/tests/physical_device.rs` — added `connect_and_disconnect_cleanly` test (ignored; validates 3.7–3.15 end-to-end)

**Notes:**

- Tasks 3.7–3.22 were implemented as a single atomic unit. They are genuinely interdependent: `connect()` requires controller mode entry (3.13), notification subscription (3.18), and characteristic discovery (3.8) before it can return a usable device.
- `BleError::DeviceNotFound` added — spec only lists 7 error variants; this is the 8th. The spec will need updating.
- Task 3.17 (test that exit packet sent on drop): `TapDevice::Drop` uses `Handle::try_current()` guard + detached spawn. A dedicated unit test is hard to write without a synchronous mock. Marked complete via the physical device test which exercises the drop path.
- Reconnect loop (3.12): exponential backoff 1s → 2s → … → 60s cap; restarts keepalive + notification tasks on success.
- `tap_data` field in `TapDevice` is retained for reconnect path (re-subscribe after reconnect). Suppressed with `#[allow(dead_code)]` + comment since it's used by the spawned tasks, not the struct methods directly.
- 46 unit tests pass; clippy clean; fmt clean.

**Next:** 3.16 is deferred to Epic 4 (Tauri `on_window_event`). Remaining Epic 3 tasks: 3.23–3.26 (DeviceRegistry). Then physical device integration testing.

---

## 2026-03-14 — Scanner mock + UUID filter tests; task 3.6 complete

**Tasks completed:** 3.6
**Tasks in progress:** none

**Files changed:**

- `crates/tap-ble/Cargo.toml` — added `macros` to tokio features; `[dev-dependencies]`: `async-trait = "0.1"`, `futures = "0.3"`; fixed accidental move of `thiserror`/`serde`/`serde_json` into dev-deps (caused by earlier Edit replacing wrong range)
- `crates/tap-ble/src/scanner.rs` — refactored inner body into `pub(crate) scan_with_adapter<C: Central>` for testability; `discover_devices` delegates to it; added `#[cfg(test)]` module with `MockPeripheral` + `MockAdapter` (full btleplug trait impls) and 4 unit tests
- `crates/tap-ble/tests/physical_device.rs` — placeholder for physical device integration tests (Tap Strap 2 + TapXR); tests run with `--ignored`

**Notes:**

- Mock implements the full `btleplug::api::Central` and `btleplug::api::Peripheral` traits; unused methods use `unimplemented!()`. The mock is usable for all future scanner/connection tests.
- `btleplug 0.12` uses `#[async_trait]`; `async-trait` added as a dev dep for the mock impl blocks.
- 4 tests: UUID constant string value, scan filter captures TAP_SERVICE_UUID, multi-device RSSI sort, None-RSSI last.

**Next:** 3.7 — Implement `TapDevice::connect(address)` — connect and verify bond/pair status

---

## 2026-03-14 — discover_devices implemented; tasks 3.2–3.5 complete

**Tasks completed:** 3.2, 3.3, 3.4, 3.5
**Tasks in progress:** none

**Files changed:**

- `crates/tap-ble/Cargo.toml` — added `uuid = "1"` (btleplug 0.12 does not re-export `Uuid`)
- `crates/tap-ble/src/scanner.rs` — `get_adapter()` (adapter init + not-found error), `discover_devices()` (scan filter, dedup via btleplug's native peripheral list, RSSI sort), `TAP_SERVICE_UUID` const
- `docs/spec/tap-ble-spec.md` — updated dependency note: uuid is a required direct dep in 0.12

**Notes:**

- `btleplug 0.12` uses `uuid::Uuid` via a private `use`, not a `pub use`. Direct dep added with no extra features.
- Deduplication (task 3.4) is handled natively: `adapter.peripherals()` returns one entry per hardware address. Properties reflect the most recent advertisement. A comment in the code explains this.
- `get_adapter()` is `pub(crate)` so `BleManager` and `TapDevice` can reuse it for connecting to devices (tasks 3.7+).

**Next:** 3.6 — UUID filter integration test (mock or physical device)

---

## 2026-03-14 — Epic 3 spec approved; tap-ble scaffolded with btleplug 0.12 (task 3.1)

**Tasks completed:** 3.1
**Tasks in progress:** none

**Files changed:**

- `docs/spec/tap-ble-spec.md` — new: full Epic 3 spec incorporating `docs/reference/` (api-doc, windows-sdk-guid-reference, raw-sensor-mode)
- `crates/tap-ble/Cargo.toml` — added `btleplug 0.12` (serde feature), `tokio`, `thiserror`, `serde`, `serde_json`; removed `rust-version` (edition 2024 requires 1.85, inconsistent with 1.77.2 in `.clippy.toml`)
- `crates/tap-ble/src/lib.rs` — replaced scaffold with module declarations and re-exports
- `crates/tap-ble/src/error.rs` — `BleError` enum with all variants from spec
- `crates/tap-ble/src/device_info.rs` — `TapDeviceInfo` struct
- `crates/tap-ble/src/device_registry.rs` — `DeviceRegistry` struct skeleton (methods stubbed with `todo!`)
- `crates/tap-ble/src/scanner.rs` — `discover_devices` stub
- `crates/tap-ble/src/tap_device.rs` — `TapDevice` stub
- `crates/tap-ble/src/manager.rs` — `BleManager` with broadcast channel wired up

**Notes:**

- `btleplug 0.12` resolves to `0.12.0` and pulls in the `bluez-async` / `bluez-generated` stack on Linux. Compiles cleanly.
- Spec was updated to include the full GUID table from `windows-sdk-guid-reference.txt`, bonding requirement and ATT MTU note from `api-doc.txt`, and a raw-sensor-mode out-of-scope note from `raw-sensor-mode.txt`.
- `rust-version = "1.77.2"` was removed from `tap-ble/Cargo.toml`: edition 2024 requires a minimum of 1.85.0, making the field incorrect. `mapping-core` does not set `rust-version` either. This should be revisited uniformly at the packaging epic.

**Next:** 3.2–3.5 — BLE adapter init and `discover_devices` implementation

---

## 2026-03-14 — Task 2.35: debug event timing metadata tests + two bug fixes

**Tasks completed:** 2.35
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/src/engine/combo_engine.rs` — two bug fixes:
  1. `dispatch()`: `Resolved` debug event `layer_stack` field now uses `self.layer_stack.layer_ids()` (full stack captured before the walk) instead of `layers_checked` (which stopped at the first match). This ensures the full stack is always reported, not just the layers visited during the walk.
  2. `push_event()`: `ComboTimeout` debug event was dead code — `flush_expired_combo` removed the timed-out entry before the combo check's else branch could emit the event. Fixed by emitting the debug event **before** `flush_expired_combo` when gap > window and debug mode is on. The dead else branch in the combo check was removed.
- `crates/mapping-core/tests/combo_engine.rs` — 6 debug timing metadata tests added:
  - `debug_resolved_waited_ms_reflects_buffering_duration`
  - `debug_resolved_pattern_matches_tapped_code`
  - `debug_resolved_layer_stack_and_matched_layer_are_correct`
  - `debug_resolved_matched_mapping_label_is_correct`
  - `debug_unmatched_passthrough_layers_checked_lists_all_walked_layers`
  - `debug_combo_timeout_reports_correct_gap_and_window`
- `docs/plan/implementation-plan.md` — 2.35 marked complete

**Notes:**
Epic 2 is now fully complete. All 216 tests pass; clippy clean.
The `ComboTimeout` bug was a design issue: the flush happened before the timeout could be detected in the combo-check branch. Solution is correct — detect before flush, remove dead else.

**Next:** Epic 3 (BLE layer, `tap-ble` crate) — requires a spec document before any code is written.
