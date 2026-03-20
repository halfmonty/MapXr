# tap-mapper — implementation plan

## Current focus

**Next task:** Epic 14 complete (14.5 skipped — no Apple account; 14.6 pending SignPath.io application)
**Epic:** 14 — Packaging and distribution
**Blocker:** 14.6 requires SignPath.io approval (external process); can ship without it
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

## Epic 9 — Mouse action support

> Goal: allow profile mappings to click the mouse and scroll, extending the action vocabulary.
> Spec: extend `docs/spec/mapping-core-spec.md` §Action types
> Dependencies: Epics 1, 2, 4, 5
> `enigo` is already an approved dependency and handles cross-platform mouse simulation.
> Note: mouse movement was explicitly excluded — keyboard-based cursor replacement tools cover that use case.

- [x] **9.1** _(spec first)_ Extend `mapping-core-spec.md`: define `mouse_click`, `mouse_double_click`, `mouse_scroll` action types; define `MouseButton` enum (`left`, `right`, `middle`) and `ScrollDirection` enum (`up`, `down`, `left`, `right`)
- [x] **9.2** Add `MouseButton`, `ScrollDirection` enums and the three mouse `Action` variants to `mapping-core`; write serde round-trip tests
- [x] **9.3** Implement mouse action dispatch in `apps/desktop/src-tauri` via `enigo` (alongside existing keyboard dispatch)
- [x] **9.4** Add mouse action editors to the Svelte action editor panel: button selector for click variants; direction selector for scroll
- [x] **9.5** Write integration tests for mouse action dispatch (use `enigo` in headless/test mode where possible; otherwise document manual test steps)

---

## Epic 10 — Device renaming

> Goal: allow users to assign a friendly name to a Tap device by writing to the BLE device name characteristic.
> Spec: `docs/spec/device-rename-spec.md` *(not yet written — draft and get approval before coding)*
> Dependencies: Epics 3, 4, 5

- [x] **10.1** _(spec first)_ Research the Tap BLE GATT characteristic used for setting the device name; document UUID and write protocol in `docs/spec/device-rename-spec.md`
- [x] **10.2** Implement `TapDevice::set_name(name: &str) -> Result<(), BleError>` in `tap-ble`, writing to the identified GATT characteristic
- [x] **10.3** Add `#[tauri::command] rename_device(address: String, name: String) -> Result<(), String>` in `src-tauri`; register in `invoke_handler`
- [x] **10.4** Add `renameDevice(address, name)` wrapper with JSDoc to `commands.ts`
- [x] **10.5** Add inline rename UI to the connected devices table (edit icon → text input → confirm); update `deviceStore` and persist the new name via `setName`
- [x] **10.6** Write unit tests for the `tap-ble` name-write path; document the manual hardware verification step

---

## Epic 11 — Context-aware automatic profile switching

> Goal: monitor the currently focused window / application and automatically activate a matching profile.
  Should work for each target OS, so implementation should be applied at the correct "level" of the project.
  Mobile use case can be ignored, this is only for desktop and MacOs cannot be tested for now so plan for
  implementing MacOs it at some point but now for now. Focus on Linux/Windows.
> Spec: `docs/spec/context-switching-spec.md` *(not yet written — draft and get approval before coding)*
> Dependencies: Epics 4, 5

- [x] **11.1** _(spec first)_ Design and write `docs/spec/context-switching-spec.md`: `ContextRule` schema (window class / title pattern → `layer_id`), rule evaluation order, conflict resolution, JSON storage path
- [x] **11.2** Implement per-platform focused-window monitor as a background `tokio` task in `src-tauri` (Linux: `_NET_ACTIVE_WINDOW` via `x11rb` or subprocess; macOS: `NSWorkspace`; Windows: `GetForegroundWindow`) — present platform research to user before choosing libraries
- [x] **11.3** Implement `ContextRules`: load/save `context-rules.json` from config dir; evaluate rules against the active window title/class in priority order
- [x] **11.4** Wire the monitor into `AppState`: on window focus change, evaluate rules and call `activate_profile` for the first matching rule (no-op if no match or already active)
- [x] **11.5** Add Tauri commands: `list_context_rules`, `save_context_rules`; emit `context-rule-matched` event when a rule fires
- [x] **11.6** Add context rules page to the Svelte frontend: list rules, add/edit (pattern input + profile selector), delete, drag-reorder for priority
- [x] **11.7** Write unit tests for rule evaluation logic (pattern matching, priority order, no-match case)

---

## Epic 12 — System tray and background operation

> Goal: the app collapses to the system tray on close and runs as a background service; a settings page exposes user-configurable behaviour.
> Spec: `docs/spec/system-tray-spec.md` *(not yet written — draft and get approval before coding)*
> Dependencies: Epics 4, 5

- [x] **12.1** _(spec first)_ Write `docs/spec/system-tray-spec.md`: tray icon and menu spec, window close behaviour, settings schema, per-platform start-at-login mechanism
- [x] **12.2** Add Tauri system tray plugin (`tauri-plugin-tray`); configure tray icon and context menu: Show/Hide window, active profile name (greyed out, informational), Quit
- [x] **12.3** Override window close event: instead of exiting, hide the window to tray (BLE connections remain active)
- [x] **12.4** Add tray tooltip showing active profile name and connected device count; update dynamically via Tauri state
- [x] **12.5** Create a Settings page in the Svelte frontend with options:
  - [x] **12.5a** Close button behaviour: "Minimise to tray" (default) vs "Exit"
  - [x] **12.5b** Start minimised (launch directly to tray without showing the window)
  - [x] **12.5c** Start at system login
- [x] **12.6** Persist settings to `preferences.json` (extend existing `Preferences` struct)
- [x] **12.7** Implement "start at login" per platform: Windows (registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`), macOS (`~/Library/LaunchAgents/` plist), Linux (`~/.config/autostart/` `.desktop` file)
- [x] **12.8** Wire settings changes to live behaviour without requiring restart (e.g. toggling "close to tray" takes effect immediately)

---

## Epic 13 — Design system consistency

> Goal: shared Tailwind / DaisyUI configuration so the marketing site (`apps/site`) and the desktop app (`apps/desktop`) use identical colours, typography, and component styles.
> Spec: none required — purely structural/visual work with no new data model or API surface
> Dependencies: Epics 5, 7

- [x] **13.1** Audit current Tailwind config and DaisyUI theme in both `apps/site` and `apps/desktop`; document all divergences
- [x] **13.2** Extract a shared `tailwind-base.js` (or equivalent) into a top-level `packages/design-tokens/` directory; both apps extend it
- [x] **13.3** Reconcile DaisyUI theme tokens (primary, secondary, accent, base colours) between site and app so they match visually and use 'corporate' theme for light and 'business' for dark. Should default to system preference.
- [x] **13.4** Update site pages and app pages that diverge from the shared theme; do a visual pass to confirm consistency
- [x] **13.5** Document the design system (colour palette, type scale, component conventions) in `docs/design-system.md`

---

## Epic 14 — Packaging and distribution

> Goal: installable builds for Windows, macOS, and Linux.
> Spec: none required — packaging configuration, no new data model or API surface
> Dependencies: Epics 0–13

- [x] **14.1** Configure Tauri bundler for all supported targets: `.msi` + NSIS `.exe` (Windows), `.AppImage` + `.deb` + `.rpm` (Linux); macOS skipped (no Apple Developer account)
- [x] **14.2** Add application icons in all required sizes; user to supply 1024×1024 PNG source at `apps/desktop/src-tauri/icons/source.png`; generate variants with `cargo tauri icon`
- [x] **14.3** Configure auto-updater (`tauri-plugin-updater`) pointing to GitHub Releases `latest.json`; user must generate signing keypair and set `TAURI_SIGNING_PRIVATE_KEY` GitHub secret; user must fill in repo URL + pubkey placeholders in `tauri.conf.json`
- [x] **14.4** Unified `release.yml` workflow: triggers on `v*` tags, builds Linux and Windows in parallel, publishes to a single GitHub Release; pre-release detection via tag suffix (e.g. `-beta`); individual `build-linux.yml` / `build-windows.yml` retain `workflow_dispatch` for dev builds only
- [ ] **14.5** macOS code signing — SKIPPED (no Apple Developer account; macOS builds not supported)
- [ ] **14.6** Windows code signing — recommended path: SignPath.io free open-source program (EV-equivalent, bypasses SmartScreen immediately, no cost); alternative: Certum OV cert (~$30/yr, requires reputation build-up)
- [ ] **14.7** Write a `CHANGELOG.md` and establish a versioning convention (semver: breaking profile schema changes = major)
- [ ] **14.8** Write a user-facing `README.md` covering installation, first-time setup, and a quick-start profile example

### 14b — In-app update notifications

- [x] **14.9** Add `tauri-plugin-updater` dependency; implement `check_for_update` Tauri command returning `UpdateInfo | null`
- [x] **14.10** Background `run_update_checker` task: 5 s startup delay, then checks every 24 h; emits `update-available` event when a newer version is found
- [x] **14.11** Dismissible `UpdateBanner` component in the layout; persists dismissed version in `localStorage`; "View update" button opens the dialog
- [x] **14.12** `UpdateDialog` component: release notes, download progress bar, "Install & Restart" button; `download_and_install_update` command emits `update-download-progress` events then calls `app.restart()`
- [x] **14.13** "Check for updates" tray menu item + manual "Check now" button in Settings page; both use `check_for_update` command and surface results via `updateStore`

---

## Epic 15 — Android port

> Goal: the same UI and mapping logic running on Android via a Kotlin BLE bridge and WebView host.
> Spec: `docs/spec/android-spec.md` *(not yet written — draft and get approval before coding)*
> Dependencies: Epics 1, 2 (mapping-core), 5, 6, 7 (frontend)

- [ ] **15.1** Compile `mapping-core` to an Android-compatible `.so` using `cargo-ndk`; target `aarch64-linux-android` and `armv7-linux-androideabi`
- [ ] **15.2** Write a Kotlin `MappingCoreWrapper` JNI class exposing `pushEvent(tapCode: Int, deviceId: String): String` (returns JSON-encoded `Vec<Action>`)
- [ ] **15.3** Write a Kotlin BLE manager implementing the same controller-mode protocol as `tap-ble`
- [ ] **15.4** Set up an Android project with a `WebView` hosting the Svelte build from `src/`
- [ ] **15.5** Implement a `JavascriptInterface` bridge so the Svelte frontend can invoke Kotlin commands and receive events, mirroring the Tauri command/event API
- [ ] **15.6** Adapt the Svelte `commands.ts` and `events.ts` modules to detect the runtime (Tauri vs Android WebView) and route accordingly
- [ ] **15.7** Implement the Android BLE permissions flow (runtime permission requests for `BLUETOOTH_SCAN`, `BLUETOOTH_CONNECT`)
- [ ] **15.8** Test on a physical Android device; verify combo window timing is not degraded by WebView bridge latency
- [ ] **15.9** Publish to Google Play as a free app or provide an APK download on GitHub Releases

---

## Epic 16 — Event notifications

> Goal: surface OS desktop notifications for key app events (device connected/disconnected, profile/layer switch); each notification type is individually toggleable in Settings.
> Spec: `docs/spec/notifications-spec.md`
> Dependencies: Epics 4, 5, 12 (`tauri-plugin-notification` already added in Epic 12)

- [x] **16.1** _(spec first)_ Write `docs/spec/notifications-spec.md`: enumerate notification events, payload format, per-event toggle schema, Settings UI placement
- [x] **16.2** Extend `Preferences` with per-event notification flags (`notify_device_connected`, `notify_device_disconnected`, `notify_layer_switch`); persist alongside existing preferences
- [x] **16.3** Wire notification dispatch into the BLE event pipeline: emit OS notification on device connect/disconnect when the relevant toggle is enabled
- [x] **16.4** Wire notification dispatch into the engine event pipeline: emit OS notification on profile/layer switch when the relevant toggle is enabled
- [x] **16.5** Add notification toggle controls to the Settings page (one toggle per event type); changes take effect immediately without restart
- [x] **16.6** Write unit tests for the notification-gating logic (enabled/disabled flag correctly suppresses or allows emission)

---

## Epic 17 — Extended keyboard key support

> Goal: expand the set of bindable keyboard keys to include function keys (F1–F24), media keys (play/pause, next/prev track, volume up/down/mute), and other OS-level keys (e.g. Insert, Print Screen, Scroll Lock, Pause, App menu) that a user may want to bind a tap to even if absent from their physical keyboard.
> Spec: extend `docs/spec/mapping-core-spec.md` §Key enum
> Dependencies: Epics 1, 2, 4, 5 (touches mapping-core Key type, dispatch layer, and key-picker UI)

- [x] **17.1** _(spec first)_ Audit `enigo`'s supported key set; extend the `Key` enum in `mapping-core-spec.md` to cover all viable keys: F1–F24, media keys (`MediaPlayPause`, `MediaNextTrack`, `MediaPrevTrack`, `MediaStop`, `VolumeUp`, `VolumeDown`, `VolumeMute`), and niche keys (`Insert`, `PrintScreen`, `ScrollLock`, `Pause`, `Menu`, `Sleep`, `BrowserBack`, `BrowserForward`, `BrowserRefresh`, `BrowserHome`); note any keys unsupported on specific platforms
- [x] **17.2** Add the new key variants to the `Key` enum in `mapping-core`; update serde round-trip tests
- [x] **17.3** Map new key variants to `enigo::Key` in the Tauri dispatch layer; document any platform gaps
- [x] **17.4** Update the key-picker UI in the Svelte action editor to expose the new keys, grouped by category (Standard, Function, Media, System)
- [x] **17.5** Write serde round-trip tests for all new key variants; document any keys that cannot be dispatched on a given platform

---

## Epic 18 — Haptic feedback

> Goal: send vibration patterns to connected Tap Strap / TapXr devices in response to events (tap received, layer switch, profile switch) and via an explicit action type; globally toggleable in Settings.
> Spec: `docs/spec/haptics-spec.md`
> Dependencies: Epics 3, 4, 5 (tap-ble BLE write path; mapping-core action types)

- [x] **18.1** _(spec first)_ Research the Tap BLE characteristic and command format used to trigger device vibration; write `docs/spec/haptics-spec.md` covering: GATT UUID, payload encoding, supported pattern primitives (duration, intensity where available), event-triggered use cases, and the `vibrate` action type schema
- [x] **18.2** Implement `TapDevice::vibrate(pattern: &VibrationPattern) -> Result<(), BleError>` in `tap-ble`; define `VibrationPattern` as a sequence of on/off durations in milliseconds
- [x] **18.3** Add `Action::Vibrate { pattern: VibrationPattern }` to `mapping-core`; update action dispatch in `src-tauri` to call `TapDevice::vibrate` on the relevant device(s); write serde round-trip tests
- [x] **18.4** Add a `vibrate` action editor to the Svelte action editor panel: visual pulse-sequence builder (add/remove on-off segments, duration sliders)
- [x] **18.5** Extend `Preferences` with `haptics_enabled` global toggle; wire it so all vibration dispatch is gated on this flag
- [x] **18.6** Add event-driven haptic triggers (configurable per-event, gated on `haptics_enabled`): tap received (short pulse), layer switch (distinct pattern), profile switch (distinct pattern)
- [x] **18.7** Add haptics controls to the Settings page: global enable/disable toggle; per-event enable/disable toggles
- [x] **18.8** Write unit tests for `VibrationPattern` serialisation; document manual hardware verification steps

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
Epics 0–8 (complete)
  └── Epic 9 (mouse actions)      ← extends Epic 1+2+4+5
  └── Epic 10 (device rename)     ← extends Epic 3+4+5
  └── Epic 11 (context switching) ← extends Epic 4+5
  └── Epic 12 (system tray)       ← extends Epic 4+5

Epic 13 (design system)     ← depends on Epics 5+7 being stable
Epic 14 (packaging)         ← depends on Epics 0–13 being stable
Epic 15 (Android)           ← depends on Epic 1+2 (mapping-core) and Epic 5+6+7 (frontend)
Epic 16 (notifications)     ← depends on Epics 4+5+12
Epic 17 (extended keys)     ← depends on Epics 1+2+4+5
Epic 18 (haptics)           ← depends on Epics 3+4+5
```

Epics 16, 17, and 18 can all proceed in parallel with each other and with Epic 14/15.
Epic 17 (extended keys) has no new dependency beyond what Epics 9–12 already established.
