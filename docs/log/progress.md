## 2026-03-20 — Epic 14 complete: in-app update UI, README, icons

**Tasks completed:** 14.8, 14.9, 14.10, 14.11, 14.12, 14.13
**Tasks in progress:** none

**Files changed:**

- `apps/desktop/src-tauri/src/events.rs` — added `UPDATE_AVAILABLE`, `UPDATE_DOWNLOAD_PROGRESS` constants; `UpdateAvailablePayload`, `UpdateProgressPayload` structs
- `apps/desktop/src-tauri/src/commands.rs` — added `UpdateInfoDto`, `check_for_update` command, `download_and_install_update` command (emits progress events, calls `app.restart()`)
- `apps/desktop/src-tauri/src/lib.rs` — added `TRAY_ITEM_CHECK_UPDATES` tray menu item; `trigger_update_check` helper; `run_update_checker` background task (5 s delay, 24 h interval); registered both new commands in `invoke_handler`
- `apps/desktop/src/lib/types.ts` — added `UpdateInfo`, `UpdateProgressPayload` interfaces
- `apps/desktop/src/lib/commands.ts` — added `checkForUpdate()`, `downloadAndInstallUpdate()` wrappers
- `apps/desktop/src/lib/events.ts` — added `update-available` and `update-download-progress` listeners wired to `updateStore`
- `apps/desktop/src/lib/stores/updates.svelte.ts` — new `UpdateStore` class: `available`, `dismissed` (persisted to `localStorage`), `downloading`, `progress`, `downloadError`; `shouldShow` derived getter
- `apps/desktop/src/lib/components/UpdateBanner.svelte` — new dismissible banner rendered above page content in layout
- `apps/desktop/src/lib/components/UpdateDialog.svelte` — new DaisyUI `<dialog>` modal: release notes, progress bar, Install & Restart / Not now buttons
- `apps/desktop/src/routes/+layout.svelte` — integrated `UpdateBanner` + `UpdateDialog`; fixed pre-existing `onMount` async/cleanup type error
- `apps/desktop/src/routes/settings/+page.svelte` — added Updates section with manual "Check now" button
- `README.md` — added Installation, First-time setup, Context Rules, Settings, System Tray, and updated Actions table and Build section; fixed "mapxr" → "MapXr" in prose

**Notes:**
- `download_and_install_update` uses `Arc<AtomicU64>` for the downloaded-bytes counter because `tauri-plugin-updater`'s `on_chunk` callback is `Fn` (not `FnMut`), so mutation must go through atomics.
- `app.restart()` returns `!` in Tauri v2, so the command function diverges there — `Result<(), String>` is satisfied without an unreachable `Ok(())`.
- The pre-existing `onMount(async () => ... return cleanup)` type error in `+layout.svelte` was fixed by splitting into a synchronous `onMount` that fires an IIFE internally and returns the cleanup via a closure variable.
- The backdrop close in `UpdateDialog` is intentionally disabled during download (`disabled={updateStore.downloading}`) so the user can't accidentally cancel a mid-flight install.

**Next:** Epic 14 complete. 14.6 (Windows code signing via SignPath.io) requires an external application process. Remaining stretch goals: S.1–S.5.

---

## 2026-03-19 — Epic 14 started: bundler config, auto-updater, release workflow, CHANGELOG

**Tasks completed:** 14.1, 14.2, 14.3, 14.4, 14.7
**Tasks in progress:** none

**Files changed:**

- `apps/desktop/src-tauri/tauri.conf.json` — added bundle metadata (publisher, copyright, category, descriptions, linux deb deps, windows wix/nsis config); added `plugins.updater` config with placeholder pubkey and endpoint
- `apps/desktop/src-tauri/Cargo.toml` — added `tauri-plugin-updater = "2"`; fixed description and authors fields
- `apps/desktop/src-tauri/capabilities/default.json` — added `updater:default` permission
- `apps/desktop/src-tauri/src/lib.rs` — registered `tauri_plugin_updater::Builder::new().build()` plugin
- `.github/workflows/build-linux.yml` — removed `v*` tag trigger; now `workflow_dispatch` only (dev builds)
- `.github/workflows/build-windows.yml` — same
- `.github/workflows/release.yml` — new unified release workflow: triggers on `v*` tags; two parallel jobs (Linux: appimage+deb+rpm; Windows: msi+nsis); Rust cache via `Swatinem/rust-cache@v2`; npm cache; signing env vars from GitHub Secrets; pre-release detection via tag suffix containing `-`
- `CHANGELOG.md` — created at repo root; documents semver convention; placeholder for all features built so far listed under `[Unreleased]`
- `docs/plan/implementation-plan.md` — marked 14.1, 14.3, 14.4, 14.7 complete; noted macOS (14.5) skipped; noted Windows signing (14.6) recommendation

**Notes:**
- macOS is explicitly skipped — no Apple Developer account, no Mac hardware to test on. The `"targets": "all"` in tauri.conf.json is correct since it builds whatever is available on the current runner OS, so Linux and Windows runners produce the right artifacts with no macOS-specific config needed.
- The updater plugin requires two manual steps by the user before it is functional: (1) generate a keypair with `cargo tauri signer generate -w ~/.tauri/mapxr.key` and replace the `REPLACE_WITH_YOUR_PUBLIC_KEY` placeholder in `tauri.conf.json` with the public key output; (2) add `TAURI_SIGNING_PRIVATE_KEY` (and optionally `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) as GitHub repository secrets. Without these steps, the release workflow still builds and publishes installers — it just won't produce a signed `latest.json` for the updater endpoint.
- The GitHub repo URL placeholder in `tauri.conf.json` (`REPLACE_WITH_YOUR_GITHUB_USERNAME`) also needs to be filled in.
- The two existing dev-build workflows retain `workflow_dispatch` so you can still trigger test builds without pushing a tag.
- Pre-release detection: any tag containing `-` (e.g. `v1.0.0-beta.1`) is marked as a GitHub pre-release automatically.

**Next:** 14.2 — complete (icons generated); 14.8 — user-facing README

---

## 2026-03-20 — Post-ship bug fixes: context monitor idempotency + haptic payload padding

**Tasks completed:** (bug fixes, no new task IDs)
**Tasks in progress:** none

**Files changed:**

- `apps/desktop/src-tauri/src/pump.rs` — added idempotency guard in `run_context_monitor` to skip profile activation when `last_active_profile_id` already matches the rule target, preventing repeated haptic-on-profile-switch firings on Wayland window title changes
- `crates/mapping-core/src/types/action.rs` — `VibrationPattern::encode()` now always returns a fixed 20-byte payload (2-byte header + 18 zero-padded duration slots) matching the C# SDK; previously short payloads caused the device to read uninitialised RAM as phantom durations, producing multiple unexpected buzzes
- `crates/tap-ble/tests/physical_device.rs` — added `vibrate_pattern_hardware` manual test; removed `vibrate_raw` diagnostic test
- `docs/decisions.md` — added entry for the 20-byte haptic payload requirement

**Notes:**
- The phantom-buzz bug was traced by adding diagnostic logging that confirmed a single BLE notification and a single software dispatch per tap. The root cause was found by comparing against the C# SDK reference implementation, which always zero-initialises a 20-byte buffer.
- The Wayland context-monitor bug fired because `tokio::watch::Sender::send()` always increments the version counter, so `changed()` fires even when the focused window value is identical (title update on the same window).

**Next:** no scheduled tasks — review stretch goals or raise new requirements with user

---

## 2026-03-19 — Epic 18 complete: VibrationPattern serde tests + hardware verification doc

**Tasks completed:** 18.8 (Epic 18 fully complete)
**Tasks in progress:** none

**Files changed:**

- `crates/mapping-core/src/types/action.rs` — added 5 standalone `VibrationPattern` serde tests: `serialises_as_json_array`, `deserialises_from_json_array`, `empty_round_trips_as_empty_array`, `boundary_values_round_trip`, `single_element_round_trips`
- `docs/spec/haptics-spec.md` — added §Manual hardware verification with 8 test scenarios covering: basic vibrate action, per-event triggers, master toggle disable, empty-pattern no-op, 18-element truncation, no-device-connected silent drop; updated status to Approved
- `docs/plan/implementation-plan.md` — marked 18.8 complete; updated current focus to "none — all epics complete"

**Notes:**
- `VibrationPattern` serialises as a plain JSON array (no wrapper object) because it derives `Serialize`/`Deserialize` on a newtype struct over `Vec<u16>`. The serde tests explicitly confirm this contract (e.g. `[200,100,200]` not `{"0":[200,100,200]}`).
- Epic 18 is the last scheduled epic in the implementation plan. The stretch goals (S.1–S.4) remain unscheduled.

**Next:** no scheduled tasks — review stretch goals or raise new requirements with user

---

## 2026-03-19 — Epic 18 tasks 18.5–18.7: haptic preferences, event triggers, settings UI

**Tasks completed:** 18.5, 18.6, 18.7
**Tasks in progress:** none

**Files changed:**

- `apps/desktop/src-tauri/src/state.rs` — added `haptics_enabled` (default true), `haptic_on_tap` (false), `haptic_on_layer_switch` (true), `haptic_on_profile_switch` (true) to `StoredPreferences` and `Preferences`; updated `Default`, `load`, `save`; added 6 unit tests (defaults + backwards-compat loading)
- `apps/desktop/src-tauri/src/commands.rs` — extended `TrayPreferences` with 4 haptic fields; updated `get_preferences` and `save_preferences`; wired `maybe_haptic_on_profile_switch` and `maybe_haptic_on_layer_switch` into `activate_profile`, `push_layer`, `pop_layer` command handlers
- `apps/desktop/src-tauri/src/pump.rs` — added `PATTERN_SHORT_PULSE`, `PATTERN_DOUBLE_PULSE`, `PATTERN_TRIPLE_PULSE` constants; added `vibrate_pattern` helper; added `maybe_haptic_on_tap`, `maybe_haptic_on_layer_switch`, `maybe_haptic_on_profile_switch`; gated `Action::Vibrate` explicit dispatch on `haptics_enabled`; wired `maybe_haptic_on_tap` after non-empty outputs in both `push_event` and `check_timeout` paths; wired `maybe_haptic_on_layer_switch` in `PushLayer`/`PopLayer`/`SwitchLayer` execute arms and `process_outputs` engine-side layer_changed path; wired `maybe_haptic_on_profile_switch` in context monitor
- `apps/desktop/src/lib/types.ts` — extended `TrayPreferences` with 4 haptic fields
- `apps/desktop/src/routes/settings/+page.svelte` — added haptic initial state; added "Haptics" section with master toggle and 3 per-event toggles (per-event toggles visually greyed out when master is off)

**Notes:**
- The per-event haptic toggles dim (`opacity-40 pointer-events-none`) when `haptics_enabled` is false — saves an extra round-trip; the backend also gates each call independently so there is no race condition
- `haptic_on_tap` fires before `process_outputs` so the device gets the short pulse immediately on gesture resolution, before any key simulation or layer action runs

**Next:** 18.8 — `VibrationPattern` serde round-trip tests; manual hardware verification steps

---

## 2026-03-19 — Epic 18 task 18.4: vibrate action editor

**Tasks completed:** 18.4
**Tasks in progress:** none

**Files changed:**

- `apps/desktop/src/lib/types.ts` — added `VibrationPattern = number[]` type; added `{ type: "vibrate"; pattern: VibrationPattern }` to `Action` union
- `apps/desktop/src/lib/components/ActionEditor.svelte` — added `"vibrate"` to `ALL_TYPES` and `TYPE_LABELS`; added `defaultAction` case (default `[200, 100, 200]`); added `VIBRATE_PRESETS` constants (short/double/triple pulse from spec); added `vibrateSetPattern`, `vibrateAddSegment`, `vibrateRemoveSegment`, `vibrateUpdateSegment` helpers; added vibrate UI section with preset buttons, per-segment on/off labels with number inputs, add/remove controls, 18-segment cap message, and empty-pattern warning
- `apps/desktop/src/lib/components/ActionSummary.svelte` — added `vibrate` summary (`Vibrate [200, 100, 200]`); also added missing `hold_modifier` and `conditional` cases that were pre-existing omissions

**Notes:**
- Duration inputs use `step=10`, `min=10`, `max=2550` matching the BLE encoding constraints; `vibrateUpdateSegment` also clamps and rounds to nearest 10ms client-side for immediate feedback
- Segment labels ("On 1", "Off 1", "On 2", …) are derived from index parity so they stay correct after removes
- The only TypeScript error in svelte-check is the pre-existing `+layout.svelte` async onMount issue, unchanged from before

**Next:** 18.5 — extend `Preferences` with `haptics_enabled` + per-event flags; wire global gate

---

## 2026-03-19 — Epic 18 tasks 18.1–18.3: haptics spec, BLE vibrate, Action::Vibrate

**Tasks completed:** 18.1, 18.2, 18.3
**Tasks in progress:** none

**Files changed:**

- `docs/spec/haptics-spec.md` — new: GATT characteristic C3FF0009, payload format, VibrationPattern encoding rules, vibrate action schema, event-triggered patterns, built-in pattern constants, Settings schema, task breakdown; sources cited from `gatt-characteristics.txt` and `vibration.txt`
- `crates/mapping-core/src/types/action.rs` — added `VibrationPattern` struct with `encode()` method; added `Action::Vibrate { pattern }` variant; updated doc comment; added 12 encoding tests + 4 serde round-trip tests
- `crates/mapping-core/src/types/mod.rs` — exported `VibrationPattern`
- `crates/tap-ble/src/tap_device.rs` — added `HAPTIC_UUID` constant (C3FF0009); implemented `TapDevice::vibrate()`; imports `VibrationPattern` from `mapping_core`; added `haptic_uuid_is_correct` test and hardware verification comments
- `crates/tap-ble/src/manager.rs` — added `BleManager::vibrate_all()` which iterates connected devices and calls `vibrate()`; errors per-device are logged and do not abort the loop
- `crates/tap-ble/src/lib.rs` — no re-export of VibrationPattern (canonical home is mapping-core)
- `apps/desktop/src-tauri/src/pump.rs` — added `Action::Vibrate` arm in `execute_action` (calls `ble_manager.vibrate_all`); added `"vibrate"` to `action_kind_name`
- `docs/plan/implementation-plan.md` — marked 18.1–18.3 complete; updated current focus to 18.4

**Notes:**
- `VibrationPattern` lives in `mapping-core` (not `tap-ble`) because it is part of the `Action` type. `tap-ble` imports it from `mapping_core::types`. The BLE encoding (`encode()`) is also in `mapping-core` — it is pure arithmetic with no BLE-specific types, so this does not couple the core to the BLE layer.
- `vibrate_all` dispatches to all connected devices; per-device errors are logged at `warn` but don't fail the action — consistent with how key simulation errors are handled.
- The `haptics_enabled` global gate (task 18.5) is not yet in place; vibration fires unconditionally for now.

**Next:** 18.4 — add vibrate action editor to the Svelte action editor panel (pattern builder: add/remove segments, duration inputs)

---

## 2026-03-19 — Epic 17 complete: extended keyboard key support (17.1–17.5)

**Tasks completed:** 17.1, 17.2, 17.3, 17.4, 17.5
**Tasks in progress:** none

**Files changed:**

- `docs/spec/extended-keys-spec.md` — new: full audit, canonical key list, enigo mapping table, platform availability matrix, implementation task breakdown
- `docs/spec/mapping-core-spec.md` — §Key enum updated: flat list replaced with grouped table; added platform-limited key note and cross-reference to extended-keys-spec.md
- `crates/mapping-core/src/types/key_def.rs` — `VALID_KEYS`: added `media_stop`, `pause`, `brightness_down`, `brightness_up`, `eject`, `mic_mute`; fixed comments; added 8 new tests covering new keys, serde round-trips, and rejection of old broken names (`left`/`F1` etc.)
- `apps/desktop/src-tauri/src/pump.rs` — rewrote `name_to_key`: fixed bug 1 (arrow keys: `"left"` → `"left_arrow"`), bug 2 (F-keys: `"F1"` → `"f1"`), bugs 3–6 (F13–F24, punctuation, system keys, media/volume all now mapped); added all new keys using `#[cfg]` on match arms for platform-specific variants
- `apps/desktop/src/lib/types.ts` — replaced flat `KNOWN_KEY_NAMES` array with `KEY_GROUPS` (grouped structure with `platformNote`); `KNOWN_KEY_NAMES` is now derived from it
- `apps/desktop/src/lib/components/ActionEditor.svelte` — key picker for `key` action switched from text input + datalist to `<select>` with `<optgroup>` groups; key chord datalist updated to use `KEY_GROUPS`

**Notes:**
- Six bugs were found during audit: arrow keys, F-keys (case), F13–F24, punctuation, locking/system keys, and media/volume keys were all declared valid in `VALID_KEYS` but silently fired nothing due to missing/wrong `name_to_key` arms.
- Platform-specific keys use `#[cfg]` on match arms — on unsupported platforms those arms are simply absent so the key name falls through to the `other` catch-all (logs warn, returns None). No runtime platform check needed.
- `f21`–`f24` are not available on macOS (enigo constraint). All others are cross-platform or limited as documented in the spec.
- The pre-existing `+layout.svelte` TypeScript error (async onMount return type mismatch) is not introduced by this epic.
- Windows-only keys (`menu`, `sleep`, `browser_*`) were excluded — enigo supports them only on Windows, making them poor cross-platform additions.

**Next:** 18.1 — research Tap BLE vibration command format; write `docs/spec/haptics-spec.md` (spec-first, needs approval before 18.2)

---

## 2026-03-19 — QoL: device name in notifications + save_profile hot-reload

**Tasks completed:** (out-of-band bugfixes/QoL, no plan task IDs)
**Tasks in progress:** none

**Files changed:**

- `crates/tap-ble/src/tap_device.rs` — added `pub async fn name() -> Option<String>` to `TapDevice`; populated `name` field in `BleStatusEvent::Disconnected` (unexpected drop) and `BleStatusEvent::Connected` (auto-reconnect)
- `crates/tap-ble/src/manager.rs` — added `name: Option<String>` to both `BleStatusEvent` variants; populated at all four creation sites (`connect`, `disconnect`, `reassign_role` × 2); get name before `insert`/`remove` so peripheral is still held
- `apps/desktop/src-tauri/src/pump.rs` — added `device_label(name, role)` helper producing `"Name (Role)"` or `"Role"` fallback; updated notification body to use it; updated `ble_status_to_event` match arms to use `..`
- `apps/desktop/src-tauri/src/commands.rs` — `save_profile` now takes `app: tauri::AppHandle`; after registry reload, checks if saved `layer_id` is in the current engine stack and calls `set_profile` + emits `layer-changed` if so (hot-reload without deactivate/reactivate)

**Notes:**
- `BleStatusEvent` is a public type in `tap-ble`; adding the `name` field is additive but required updating all match arms. `ble_status_to_event` in pump.rs now uses `..` on both arms.
- `save_profile` hot-reload: if a pushed layer is saved, `set_profile` replaces the base and clears the stack — acceptable since the profile definition changed.
- The `libayatana-appindicator` deprecation warning in the terminal is an upstream Tauri tray issue on Linux; not actionable from our code.

**Next:** 17.1 — audit enigo key set; extend `mapping-core-spec.md` §Key enum with F1–F24, media keys, and system keys (spec-first, needs approval before 17.2)

---

## 2026-03-19 — Epic 16 complete: OS desktop notifications (16.1–16.6)

**Tasks completed:** 16.1, 16.2, 16.3, 16.4, 16.5, 16.6
**Tasks in progress:** none

**Files changed:**

- `docs/spec/notifications-spec.md` — new: notification events, payload format, defaults, Settings UI placement
- `apps/desktop/src-tauri/src/state.rs` — added `notify_device_connected`, `notify_device_disconnected`, `notify_layer_switch`, `notify_profile_switch` to `StoredPreferences` and `Preferences`; updated `Default`, `load`, `save`; added 5 unit tests
- `apps/desktop/src-tauri/src/lib.rs` — added `pub(crate) fn send_notification` helper (best-effort, logs warn on failure)
- `apps/desktop/src-tauri/src/pump.rs` — added `capitalize_role`, `maybe_notify_layer_switch`, `maybe_notify_profile_switch` helpers; wired BLE connect/disconnect notifications into `run_ble_status_listener`; wired layer-switch notifications into `execute_action` (PushLayer, PopLayer, SwitchLayer) and `process_outputs` (engine-side `output.layer_changed`); wired profile-switch notification into `run_context_monitor`
- `apps/desktop/src-tauri/src/commands.rs` — extended `TrayPreferences` DTO with 4 notification fields; updated `get_preferences` and `save_preferences` to include them; wired profile-switch notification into `activate_profile`; wired layer-switch notifications into `push_layer` and `pop_layer` commands
- `apps/desktop/src/lib/types.ts` — extended `TrayPreferences` interface with 4 notification fields
- `apps/desktop/src/routes/settings/+page.svelte` — added "Notifications" section with 4 toggle rows; updated default state

**Notes:**
- `notify_layer_switch` defaults to `false` (can be noisy for users who switch layers often via combos)
- Profile-switch vs layer-switch distinction: `activate_profile` command and context monitor → profile_switch; PushLayer/PopLayer/SwitchLayer actions and commands → layer_switch
- `deactivate_profile` does not fire a profile-switch notification (user explicitly turned off all profiles)
- `auto_reconnect` calls `emit_layer_changed` at startup; no notification fired there (startup path)
- Notifications are best-effort: OS errors logged at warn, never propagated to UI

**Next:** 17.1 — audit enigo key set and extend `mapping-core-spec.md` §Key enum (spec-first; must be approved before 17.2)

---

## 2026-03-19 — Epic 13 complete: design system documented (13.4–13.5)

**Tasks completed:** 13.4, 13.5
**Tasks in progress:** none

**Files changed:**

- `apps/site/src/lib/components/Nav.svelte` — `LIGHT = 'corporate'` (was left as `'wireframe'`)
- `apps/site/src/app.html` — flash-prevention script: `wireframe` → `corporate`
- `docs/design-system.md` — new: colour token table, typography, spacing, component conventions, layout diagrams for desktop and site

**Notes:**
Full audit of 25 Svelte files found no hardcoded colours or non-semantic Tailwind palette classes in either app. The only fixes needed were the two `wireframe` stragglers in `apps/site` left from the earlier theme rename.

**Next:** 16.1 — write `docs/spec/notifications-spec.md` (spec-first, must be approved before any Epic 16 implementation)

---

## 2026-03-19 — Design system: shared base config + corporate/business themes; plan restructure (13.1–13.3)

**Tasks completed:** 13.1, 13.2, 13.3
**Tasks in progress:** none

**Files changed:**

- `packages/design-tokens/base.css` — new shared directory/file; placeholder for future `@theme` token overrides (see Notes)
- `apps/desktop/src/app.css` — `@plugin "daisyui" { themes: corporate, business --prefersdark; }` + `@import "../../../packages/design-tokens/base.css"`
- `apps/site/src/app.css` — same plugin config + same shared import
- `apps/desktop/src/app.html` — added flash-prevention inline script (localStorage + prefers-color-scheme → corporate/business); fixed title from "Tauri + SvelteKit + Typescript App" to "MapXr"
- `apps/site/src/app.html` — flash-prevention script updated to use corporate/business
- `apps/site/src/lib/components/Nav.svelte` — `LIGHT = 'corporate'`
- `apps/desktop/src/lib/components/TitleBar.svelte` — added theme toggle button (sun/moon icon) before window controls; reads/writes localStorage; syncs `data-theme` on `<html>` via `$effect`
- `docs/reference/project-structure.md` — added `packages/design-tokens/` entry
- `docs/plan/implementation-plan.md` — 13.1–13.3 marked complete; Epic 14 (CLI) demoted to S.5 stretch goal; old 15→14 (packaging), old 16→15 (Android); new Epic 16 (notifications), Epic 17 (extended keys), Epic 18 (haptics) added

**Notes:**
- Tailwind v4 resolves `@plugin "daisyui"` relative to the declaring CSS file, not the importing entry point. Putting `@plugin` in `packages/design-tokens/base.css` caused a build error (`Can't resolve 'daisyui'`). Fix: `@plugin "daisyui"` stays in each app's own `app.css`; the shared file is reserved for future `@theme` variable overrides.
- Pre-existing TypeScript error in `apps/desktop/src/routes/+layout.svelte` (`async onMount` returning a cleanup — Svelte type limitation) — not introduced this session.
- Light theme is `corporate`, dark theme is `business --prefersdark` in both apps.

**Next:** 13.4 — visual pass over site and app pages to confirm consistency after theme change

---

## 2026-03-19 — Frameless window + tray restoration fix

**Tasks completed:** (bugfix, no new task)
**Tasks in progress:** none

**Files changed:**

- `apps/desktop/src-tauri/tauri.conf.json` — `"decorations": false` (frameless window)
- `apps/desktop/src-tauri/src/lib.rs` — reverted all minimize/always-on-top hacks; close handler back to `win.hide()`; `toggle_window_visibility` back to `hide`/`show`
- `apps/desktop/src-tauri/capabilities/default.json` — added `core:window:allow-close`, `allow-minimize`, `allow-toggle-maximize`, `allow-internal-toggle-maximize`, `allow-start-dragging`
- `apps/desktop/src/lib/components/TitleBar.svelte` — new custom title bar: drag region, Minimize/Maximize/Close SVG buttons, DaisyUI styling (close hovers red)
- `apps/desktop/src/routes/+layout.svelte` — TitleBar added at top of layout; `windowTitle` derived from active profile name (`"MapXr"` / `"MapXr - [Profile Name]"`)

**Notes:**
After implementing the tray, restoring the window from the tray left KWin's title bar buttons (close/minimize/maximize) unresponsive until the user double-clicked the title bar. Root cause: KDE Plasma (Wayland) loses track of server-side decoration state during hide/show cycles — the same issue Discord/Electron apps avoid by using frameless windows with custom HTML title bars. Attempts to work around it via `set_always_on_top` toggle, delays, and `minimize()` instead of `hide()` all failed. The correct fix is `decorations: false` + a custom Svelte title bar, which also eliminates the problem entirely since there are no compositor-managed decorations to get confused.

**Next:** 13.1 — audit Tailwind/DaisyUI config in both apps (Epic 13)

---

## 2026-03-19 — Epic 12 complete: system tray and background operation

**Tasks completed:** 12.1, 12.2, 12.3, 12.4, 12.5 (a/b/c), 12.6, 12.7, 12.8
**Tasks in progress:** none

**Files changed:**

- `docs/spec/system-tray-spec.md` — new spec (12.1)
- `apps/desktop/src-tauri/Cargo.toml` — added `tauri` `tray-icon`+`image-png` features, `tauri-plugin-notification`, `winreg` (Windows)
- `apps/desktop/src-tauri/src/lib.rs` — complete rewrite: tray icon setup with `TrayIconBuilder`, context menu (Show/Hide/profile-label/Quit), left-click toggle, window close handler (`close_to_tray` guard with `api.prevent_close()`), `maybe_show_tray_hint()` for first-hide notification, `update_tray()` helper, `start_minimised` at startup
- `apps/desktop/src-tauri/src/state.rs` — `Preferences` extended with `close_to_tray`, `start_minimised`, `start_at_login`, `shown_tray_hint`; `preferences` field added to `AppState`; `version` bumped to 2; load/save updated; old struct-literal constructions replaced with mutation of stored prefs
- `apps/desktop/src-tauri/src/commands.rs` — `TrayPreferences` DTO; `get_preferences` and `save_preferences` commands; all previous `Preferences { .. }` literals converted to `state.preferences.lock().await` mutations
- `apps/desktop/src-tauri/src/login_item.rs` — new module: `set_start_at_login(bool)` per platform (Linux: `~/.config/autostart/*.desktop`; macOS: `~/Library/LaunchAgents/*.plist`; Windows: `HKCU\...\Run` via winreg)
- `apps/desktop/src-tauri/src/pump.rs` — `emit_layer_changed` now calls `update_tray_from_state`; `run_ble_status_listener` also calls it after each event; `Manager` import added
- `apps/desktop/src-tauri/capabilities/default.json` — added `notification:default` permission
- `apps/desktop/src/lib/types.ts` — `TrayPreferences` interface
- `apps/desktop/src/lib/commands.ts` — `getPreferences()` and `savePreferences()` wrappers
- `apps/desktop/src/routes/settings/+page.svelte` — new Settings page with three toggles
- `apps/desktop/src/routes/+layout.svelte` — "Settings" nav link added

**Notes:**
- Tray icon uses the `tray-icon` feature on the `tauri` crate (no separate `tauri-plugin-tray` crate exists)
- The first-hide notification is emitted async (spawned task) to avoid blocking the close event handler
- `try_lock` used on `ble_manager` in `update_tray_from_state` to avoid potential deadlock when the tray update is called while BLE is mid-operation
- Preferences are now stored in `AppState.preferences` so all callers mutate a single in-memory copy instead of constructing a new `Preferences` from scratch (which would have silently dropped new fields)

**Next:** 13.1 — audit Tailwind/DaisyUI config in `apps/site` and `apps/desktop` (Epic 13 has no spec requirement)

---

## 2026-03-19 — Epic 11 complete (task 11.7)

**Tasks completed:** 11.7 (and Epic 11 fully complete)
**Tasks in progress:** none

**Files changed:** none — tests were already written in task 11.3

**Notes:**
18 unit tests in `context_rules.rs` cover all 11.7 requirements: pattern matching (case-insensitive, substring, AND semantics), priority order (first-match wins, skips non-matching), and no-match cases (empty rules, no matching rule, already-active profile). All pass.

**Next:** 12.1 — write `docs/spec/system-tray-spec.md` (spec-first, must be approved before coding)
