---
covers: Epic 11 (context-aware automatic profile switching)
status: Approved
last-updated: 2026-03-19
---

# Context-aware automatic profile switching — specification (Epic 11)

## Table of contents

1. [Overview](#overview)
2. [ContextRule schema](#contextrule-schema)
3. [Storage](#storage)
4. [Rule evaluation](#rule-evaluation)
5. [Platform implementation](#platform-implementation)
6. [AppState integration](#appstate-integration)
7. [Tauri commands and events](#tauri-commands-and-events)
8. [Frontend](#frontend)
9. [Error handling](#error-handling)
10. [Testing strategy](#testing-strategy)

---

## Overview

When a user switches focus to a different application, mapxr automatically activates the
profile whose context rule matches the new foreground window. This lets, for example, a
coding profile activate when VS Code is focused and a gaming profile activate when a game
window is in front, without any manual profile switching.

**Scope:**
- Desktop only (Linux and Windows for initial implementation).
- macOS is architecturally planned but not implemented now — code must compile on macOS
  with a stub that logs "context switching not supported on macOS" and does nothing.
- Mobile is out of scope.
- Linux: X11 is the primary target. Wayland native support is deferred (see §Platform).

**Behaviour:**
- A background tokio task polls the OS for the focused window every **500 ms**.
- On a focus change, the first matching `ContextRule` (by list order) is evaluated.
- If a matching rule is found and its profile is not already active, `activate_profile` is
  called and a `context-rule-matched` Tauri event is emitted.
- If no rule matches, the current profile is left unchanged.

---

## ContextRule schema

Each rule maps a window identity pattern to a `layer_id`. A rule can match on application
name, window title, or both. If both fields are present, **both must match** (AND semantics).

### JSON representation

```json
{
  "name": "VS Code",
  "layer_id": "coding",
  "match_app": "code",
  "match_title": null
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | yes | Human-readable label shown in the UI |
| `layer_id` | `string` | yes | ID of the profile to activate |
| `match_app` | `string \| null` | no | Pattern matched against the application name |
| `match_title` | `string \| null` | no | Pattern matched against the window title |

At least one of `match_app` or `match_title` must be non-null. A rule with both null is
rejected at load time.

### Pattern matching

Patterns use **case-insensitive substring matching**: a pattern matches if the target string
contains the pattern as a substring (after lowercasing both). This covers the common case
("firefox" matches "Firefox", "code" matches "Code - OSS") without requiring a regex engine
or user knowledge of regex syntax.

Examples:
- `match_app: "firefox"` — matches any app whose name contains "firefox" (case-insensitive)
- `match_title: "vim"` — matches any window title containing "vim"
- Both set — both must match simultaneously

**Why not regex?** Regex would require the `regex` crate (a new dependency not yet approved).
Substring matching covers the primary use cases and can be extended to regex later if needed.
If you want regex support, say so and I will add it to the dependency request.

### What "app name" and "title" mean per platform

| Platform | `match_app` matches | `match_title` matches |
|----------|--------------------|-----------------------|
| Linux (X11) | `WM_CLASS` instance name (e.g. `"firefox"`, `"code"`) | `_NET_WM_NAME` |
| Windows | process executable name without extension (e.g. `"firefox"`, `"Code"`) | `GetWindowText` result |
| macOS (stub) | not evaluated | not evaluated |

---

## Storage

Rules are stored in `context-rules.json` in the app config directory, alongside
`devices.json` and `preferences.json`.

| OS | Path |
|----|------|
| Linux | `~/.config/mapxr/context-rules.json` |
| Windows | `%APPDATA%\mapxr\context-rules.json` |
| macOS | `~/Library/Application Support/mapxr/context-rules.json` |

### File format

```json
{
  "version": 1,
  "rules": [
    { "name": "VS Code", "layer_id": "coding", "match_app": "code", "match_title": null },
    { "name": "Firefox", "layer_id": "browsing", "match_app": "firefox", "match_title": null },
    { "name": "Terminal", "layer_id": "terminal", "match_app": "alacritty", "match_title": null }
  ]
}
```

An absent or empty `rules` array means context switching is disabled. A missing file is
treated as `{ "version": 1, "rules": [] }` — no error.

### Rust types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRule {
    pub name: String,
    pub layer_id: String,
    pub match_app: Option<String>,
    pub match_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRules {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub rules: Vec<ContextRule>,
}
```

`ContextRules` lives in a new file: `apps/desktop/src-tauri/src/context_rules.rs`.

---

## Rule evaluation

Rules are evaluated in list order. The first rule whose patterns all match is the winner.

```
for rule in rules:
    if rule.match_app is set and app_name does not contain rule.match_app (case-insensitive):
        continue
    if rule.match_title is set and window_title does not contain rule.match_title (case-insensitive):
        continue
    return Some(rule)
return None
```

If the matched `layer_id` is already the active profile, no action is taken (no redundant
`activate_profile` calls, no `context-rule-matched` event).

If the matched `layer_id` does not exist in the layer registry, log a warning and take no
action — do not error or crash.

---

## Platform implementation

The focused-window query lives in `apps/desktop/src-tauri/src/focus_monitor.rs`.

```rust
pub struct FocusedWindow {
    /// Application name (WM_CLASS instance on X11/XWayland; app_id on Wayland;
    /// process exe stem on Windows).
    pub app: String,
    /// Window title.
    pub title: String,
}
```

### Polling loop

A tokio task wakes every 500 ms. On Linux it calls `focused_window()` via
`spawn_blocking` (the X11 and Wayland calls are synchronous C bindings). On a change
from the previous result, it evaluates context rules and activates the matching profile.
The task holds a `watch::Receiver<bool>` cancel channel (same pattern as BLE tasks).

### Linux — dual backend (X11 + Wayland)

Wayland has no universal "get the focused window" API — it is fragmented by compositor.
The approach is to compile **both** backends and select at runtime based on environment
variables:

1. If `$WAYLAND_DISPLAY` is set → attempt the Wayland backend first.
2. If the Wayland backend is unavailable (compositor doesn't support the required
   protocol) or `$WAYLAND_DISPLAY` is unset → fall back to X11 via `$DISPLAY`.
3. If neither works → log info and the task exits cleanly.

#### Wayland backend — `wlr-foreign-toplevel-management`

The `wlr-foreign-toplevel-management-unstable-v1` Wayland extension protocol exposes
the full list of open toplevels (windows), each with `title`, `app_id`, and a set of
states including `activated` (focused). The monitor subscribes to toplevel events and
tracks which toplevel is currently `activated`.

**Supported compositors:** Sway, Hyprland, River, Niri, KDE Plasma ≥ 5.27, COSMIC.

**GNOME limitation:** GNOME's Mutter compositor does not implement this protocol and
has no public Wayland API for active-window queries. On a pure GNOME Wayland session,
native Wayland apps will not be detected. However, `$DISPLAY` is still set on GNOME
Wayland (pointing to XWayland), so the X11 fallback will detect XWayland-hosted apps
(which covers a significant portion of apps until the ecosystem fully migrates to
native Wayland). This limitation is documented in the UI.

**Proposed libraries:**
- `wayland-client 0.31` — Wayland client protocol implementation (smithay ecosystem,
  actively maintained)
- `wayland-protocols-wlr 0.3` — pre-generated Rust bindings for wlr Wayland extension
  protocols including `wlr-foreign-toplevel-management-unstable-v1`

#### X11 backend — `x11rb`

Query `_NET_ACTIVE_WINDOW` on the root window to get the focused window ID, then fetch
`_NET_WM_NAME` (title) and `WM_CLASS` (app) for that window.

**Proposed library:** `x11rb 0.13` — pure-Rust, safe XCB-based X11 client. Actively
maintained; modern successor to the `xcb` crate.

#### Runtime selection in code

```rust
pub fn start_linux_monitor(...) {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        if try_start_wayland_monitor(...).is_ok() {
            return;
        }
        log::info!("context switching: Wayland backend unavailable, falling back to X11");
    }
    if std::env::var_os("DISPLAY").is_some() {
        try_start_x11_monitor(...);
    } else {
        log::info!("context switching: no display available, disabled");
    }
}
```

### Windows

`GetForegroundWindow()` returns the `HWND` of the focused window. From that:
- `GetWindowTextW(hwnd)` → title
- `GetWindowThreadProcessId(hwnd, &pid)` → process ID, then
  `OpenProcess` + `QueryFullProcessImageNameW` → full exe path → `Path::file_stem()` → app name

**Proposed library: `windows 0.58`** (Microsoft's official Rust crate). Relevant
feature flags: `Win32_Foundation`, `Win32_UI_WindowsAndMessaging`,
`Win32_System_Threading`.

### macOS — stub only

Returns `None` and logs once: `"context switching: not supported on macOS in this build"`.
Compiles cleanly with no platform-specific deps.

### Conditional compilation structure

```
focus_monitor.rs
  ├── pub struct FocusedWindow
  ├── #[cfg(target_os = "linux")]
  │     ├── mod wayland  → wlr-foreign-toplevel-management
  │     └── mod x11      → x11rb _NET_ACTIVE_WINDOW
  ├── #[cfg(target_os = "windows")] mod windows → Win32 GetForegroundWindow
  └── #[cfg(target_os = "macos")]   mod macos   → None stub
```

---

## AppState integration

Add to `AppState`:

```rust
/// Rules for automatic profile activation on window focus change.
pub context_rules: Mutex<ContextRules>,

/// Absolute path to `context-rules.json`.
pub context_rules_path: PathBuf,
```

The lock ordering rule (see `state.rs` doc comment) extends to:

1. `engine`
2. `layer_registry`
3. `ble_manager`
4. `context_rules` (new — acquire last, never hold while awaiting BLE)

In `build_app_state()`, load `context-rules.json` from the config dir (missing = empty
rules, not an error). Return a cancel receiver for the context monitor task alongside the
existing `event_rx` and `status_rx`.

In `lib.rs`, spawn `focus_monitor::run_context_monitor(app_handle, state_arc, cancel_rx)`.

---

## Tauri commands and events

### Commands

**`list_context_rules() -> Result<Vec<ContextRule>, String>`**
Returns the current rule list. Triggers a re-load from disk first (same pattern as
`list_profiles`).

**`save_context_rules(rules: Vec<ContextRule>) -> Result<(), String>`**
Validates all rules (at least one pattern set per rule), writes `context-rules.json`,
reloads the in-memory `ContextRules`.

### Event

**`context-rule-matched`** — emitted on the Tauri app handle when a rule fires.

```ts
interface ContextRuleMatchedPayload {
  rule_name: string;   // the matching rule's human-readable name
  layer_id: string;    // the profile that was activated
}
```

---

## Frontend

A new page at `/context-rules` (or a tab within `/profiles`) with:

1. An ordered list of rules. Each row shows: name, app pattern, title pattern, target profile.
2. Add / edit / delete controls.
3. Drag-to-reorder (list order = priority).
4. A live status badge in the header: "Last matched: VS Code → coding profile" updated from
   the `context-rule-matched` event.

The frontend detail (exact layout) is left to task 11.5+. The critical part for this spec is
the data model and command signatures.

---

## Error handling

| Scenario | Behaviour |
|----------|-----------|
| `context-rules.json` missing on startup | Treated as empty rules — no error |
| `context-rules.json` malformed JSON | Log warning, use empty rules |
| Rule references a `layer_id` that doesn't exist | Log warning, skip rule; do not crash |
| X11 connection fails at startup (Wayland, headless) | Log info, monitor task exits; no error surfaced to UI |
| Windows `GetForegroundWindow` returns null | Treated as no focused window; skip evaluation |
| `save_context_rules` validation fails | Return `Err(message)` describing violation |

---

## Testing strategy

### Unit tests

- `context_rules_evaluate_first_match_wins` — two rules, first matches, second is not checked
- `context_rules_evaluate_app_and_title_both_must_match` — AND semantics
- `context_rules_evaluate_case_insensitive` — "FIREFOX" matches pattern "firefox"
- `context_rules_evaluate_no_match_returns_none`
- `context_rules_evaluate_already_active_returns_none`
- `context_rules_load_missing_file_returns_empty`
- `context_rules_load_malformed_json_returns_empty`
- `context_rules_validate_rule_with_no_patterns_is_rejected`

### Integration / manual tests

- Focused-window query on Linux X11: manually verify that `focused_window()` returns correct
  app and title when switching between a terminal and a browser.
- Rule firing: create a rule matching `"alacritty"`, focus Alacritty, verify profile activates
  and `context-rule-matched` event is emitted.
- Windows: same manual steps on a Windows machine.

---

## New dependency requests

The following crates are not yet approved and require your sign-off before task 11.2 begins:

| Crate | Version | Purpose | Gated by |
|-------|---------|---------|----------|
| `x11rb` | `0.13` | X11 `_NET_ACTIVE_WINDOW` query | `cfg(target_os = "linux")` |
| `wayland-client` | `0.31` | Wayland client protocol runtime | `cfg(target_os = "linux")` |
| `wayland-protocols-wlr` | `0.3` | `wlr-foreign-toplevel-management` bindings | `cfg(target_os = "linux")` |
| `windows` | `0.58` | Win32 `GetForegroundWindow` + process name | `cfg(target_os = "windows")` |

All are well-maintained, platform-gated, and add no compile-time cost on other OSes.
