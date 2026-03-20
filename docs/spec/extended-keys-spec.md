# Epic 17 — Extended keyboard key support

**Status: DRAFT — awaiting approval before any code changes**

---

## Overview

This spec covers:

1. An audit of bugs in the current key dispatch path (keys declared valid but never dispatched)
2. The complete canonical key name list after this epic
3. New keys to add beyond what was already declared
4. Platform availability notes
5. Implementation task breakdown

---

## Audit — current bugs in `pump.rs` `name_to_key`

The following keys appear in `VALID_KEYS` in `key_def.rs` (so profiles accept them without error)
but have **no matching arm in `name_to_key`** in `pump.rs`. They silently log a warning and
do nothing when dispatched.

### Bug 1 — Arrow keys: wrong string literals

`VALID_KEYS` uses `"left_arrow"`, `"right_arrow"`, `"up_arrow"`, `"down_arrow"`.
`name_to_key` matches `"left"`, `"right"`, `"up"`, `"down"`.
**Result: all four arrow keys are currently silently ignored when fired.**

Fix: change the match arms in `name_to_key` to use the `_arrow` suffix.

### Bug 2 — Function keys F1–F12: wrong case

`VALID_KEYS` uses lowercase `"f1"`–`"f12"`.
`name_to_key` matches uppercase `"F1"`–`"F12"`.
**Result: every function key fires nothing.**

Fix: change match arms to lowercase.

### Bug 3 — F13–F24: defined but never dispatched

`VALID_KEYS` includes `"f13"`–`"f24"` but `name_to_key` has no arms for them at all.

Fix: add arms for `"f13"`–`"f20"` (all platforms) and `"f21"`–`"f24"` (Windows/Linux only; see
platform table below).

### Bug 4 — Punctuation keys: defined but never dispatched

All of the following are in `VALID_KEYS` but absent from `name_to_key`:

`"grave"`, `"minus"`, `"equals"`, `"left_bracket"`, `"right_bracket"`, `"backslash"`,
`"semicolon"`, `"quote"`, `"comma"`, `"period"`, `"slash"`

Fix: map each to `Key::Unicode(char)` using the unshifted character (see table below).

### Bug 5 — Locking / system keys: defined but never dispatched

`"caps_lock"`, `"insert"`, `"num_lock"`, `"scroll_lock"`, `"print_screen"` are all in
`VALID_KEYS` but have no `name_to_key` arm.

Fix: map each to its enigo variant (platform limits apply; see table).

### Bug 6 — Media / volume keys: defined but never dispatched

`"media_play"`, `"media_next"`, `"media_prev"`, `"volume_up"`, `"volume_down"`,
`"volume_mute"` are in `VALID_KEYS` but missing from `name_to_key`.

Fix: add arms (all cross-platform except `media_stop`; see table).

---

## New keys to add

The following keys are **not currently in `VALID_KEYS`** and should be added.

| Key name | Use case |
|---|---|
| `media_stop` | Stop media playback; commonly mapped on extended keyboards |
| `pause` | Pause/Break key; used by some IDEs and games |
| `brightness_down` | Screen brightness (macOS; useful for system-level layer profiles) |
| `brightness_up` | Screen brightness (macOS) |
| `mic_mute` | Microphone mute (Linux); increasingly common in video-call setups |
| `eject` | Eject disc (macOS; niche but present on older Macs) |

The following Windows-only system keys from the plan are **excluded** from this epic:
`menu`, `sleep`, `browser_back`, `browser_forward`, `browser_refresh`, `browser_home`.
These are Windows-only in enigo 0.2, making them poor cross-platform targets. They can be
revisited in a later epic if Windows-specific profile support becomes a priority.

---

## Complete canonical key name list

After this epic, `VALID_KEYS` contains exactly the following strings (sorted, as required
for `binary_search`):

### Letters (a–z)
`"a"` `"b"` `"c"` `"d"` `"e"` `"f"` `"g"` `"h"` `"i"` `"j"` `"k"` `"l"` `"m"`
`"n"` `"o"` `"p"` `"q"` `"r"` `"s"` `"t"` `"u"` `"v"` `"w"` `"x"` `"y"` `"z"`

### Digits (0–9)
`"0"` `"1"` `"2"` `"3"` `"4"` `"5"` `"6"` `"7"` `"8"` `"9"`

### Function keys
`"f1"` through `"f24"`

### Navigation
`"backspace"` `"caps_lock"` `"delete"` `"down_arrow"` `"end"` `"escape"` `"home"`
`"insert"` `"left_arrow"` `"num_lock"` `"page_down"` `"page_up"` `"print_screen"`
`"return"` `"right_arrow"` `"scroll_lock"` `"space"` `"tab"` `"up_arrow"`

### Punctuation (unshifted)
`"backslash"` `"comma"` `"equals"` `"grave"` `"left_bracket"` `"minus"` `"period"`
`"quote"` `"right_bracket"` `"semicolon"` `"slash"`

### Media
`"media_next"` `"media_play"` `"media_prev"` `"media_stop"`

### Volume
`"volume_down"` `"volume_mute"` `"volume_up"`

### System (platform-limited)
`"brightness_down"` `"brightness_up"` `"eject"` `"mic_mute"` `"pause"`

---

## `name_to_key` dispatch mapping

The complete mapping from key name string → `enigo::Key` variant, including platform limits.

### Letters and digits

Map via `Key::Unicode(char)`. No change to existing behaviour.

### Function keys

| Key name | `enigo::Key` variant | Win | macOS | Linux |
|---|---|:---:|:---:|:---:|
| `f1`–`f12` | `F1`–`F12` | ✓ | ✓ | ✓ |
| `f13`–`f20` | `F13`–`F20` | ✓ | ✓ | ✓ |
| `f21`–`f24` | `F21`–`F24` | ✓ | ✗ | ✓ |

`f21`–`f24` on macOS: `name_to_key` returns `None`; the pump logs a warning and skips
dispatch. Profile validation still accepts the key name — the profile itself is valid, but
firing it on macOS does nothing. This matches the pattern already used elsewhere.

### Navigation keys

| Key name | `enigo::Key` variant | Win | macOS | Linux |
|---|---|:---:|:---:|:---:|
| `backspace` | `Backspace` | ✓ | ✓ | ✓ |
| `caps_lock` | `CapsLock` | ✓ | ✓ | ✓ |
| `delete` | `Delete` | ✓ | ✓ | ✓ |
| `down_arrow` | `DownArrow` | ✓ | ✓ | ✓ |
| `end` | `End` | ✓ | ✓ | ✓ |
| `escape` | `Escape` | ✓ | ✓ | ✓ |
| `home` | `Home` | ✓ | ✓ | ✓ |
| `insert` | `Insert` | ✓ | ✗ | ✓ |
| `left_arrow` | `LeftArrow` | ✓ | ✓ | ✓ |
| `num_lock` | `Numlock` | ✓ | ✗ | ✓ |
| `page_down` | `PageDown` | ✓ | ✓ | ✓ |
| `page_up` | `PageUp` | ✓ | ✓ | ✓ |
| `print_screen` | `Print` (Linux) / `Snapshot` (Win) | ✓ | ✗ | ✓ |
| `return` | `Return` | ✓ | ✓ | ✓ |
| `right_arrow` | `RightArrow` | ✓ | ✓ | ✓ |
| `scroll_lock` | `ScrollLock` | ✗ | ✗ | ✓ |
| `space` | `Space` | ✓ | ✓ | ✓ |
| `tab` | `Tab` | ✓ | ✓ | ✓ |
| `up_arrow` | `UpArrow` | ✓ | ✓ | ✓ |

**Platform note for `print_screen`:** enigo uses `Print` on Linux and `Snapshot` on Windows;
`name_to_key` must use `#[cfg]` or a `cfg!()` branch to emit the right variant. macOS:
returns `None`.

**Platform note for `scroll_lock`:** enigo's `ScrollLock` is Linux-only (`#[cfg(all(unix,
not(target_os = "macos")))]`). On Windows and macOS the arm returns `None`.

### Punctuation keys (via `Key::Unicode`)

| Key name | Unicode char | Unshifted label |
|---|:---:|---|
| `grave` | `` ` `` | backtick / tilde key |
| `minus` | `-` | hyphen / minus |
| `equals` | `=` | equals / plus key |
| `left_bracket` | `[` | left square bracket |
| `right_bracket` | `]` | right square bracket |
| `backslash` | `\` | backslash / pipe key |
| `semicolon` | `;` | semicolon / colon key |
| `quote` | `'` | apostrophe / single quote key |
| `comma` | `,` | comma / less-than key |
| `period` | `.` | period / greater-than key |
| `slash` | `/` | forward slash / question key |

**Note:** `Key::Unicode(char)` sends the character directly. If a `Key` action specifies
a punctuation key with a modifier (e.g. `shift` + `grave` to produce `~`), the modifier
is still pressed, but the OS keyboard layout is responsible for interpreting the
combination. This is consistent with how letters and digits already work.

### Media and volume keys

| Key name | `enigo::Key` variant | Win | macOS | Linux |
|---|---|:---:|:---:|:---:|
| `media_play` | `MediaPlayPause` | ✓ | ✓ | ✓ |
| `media_next` | `MediaNextTrack` | ✓ | ✓ | ✓ |
| `media_prev` | `MediaPrevTrack` | ✓ | ✓ | ✓ |
| `media_stop` | `MediaStop` | ✓ | ✗ | ✓ |
| `volume_up` | `VolumeUp` | ✓ | ✓ | ✓ |
| `volume_down` | `VolumeDown` | ✓ | ✓ | ✓ |
| `volume_mute` | `VolumeMute` | ✓ | ✓ | ✓ |

### System keys (platform-limited)

| Key name | `enigo::Key` variant | Win | macOS | Linux |
|---|---|:---:|:---:|:---:|
| `brightness_down` | `BrightnessDown` | ✗ | ✓ | ✗ |
| `brightness_up` | `BrightnessUp` | ✗ | ✓ | ✗ |
| `eject` | `Eject` | ✗ | ✓ | ✗ |
| `mic_mute` | `MicMute` | ✗ | ✗ | ✓ |
| `pause` | `Pause` | ✓ | ✗ | ✓ |

All five return `None` from `name_to_key` on unsupported platforms; the pump logs a
warning and skips dispatch.

---

## Validation behaviour (no change)

Profile validation already calls `KeyDef::validate()` for every key name at load time.
Unknown names are rejected with a clear error message listing valid examples. This
behaviour does not change — only the content of `VALID_KEYS` grows.

Keys that are valid on all platforms (e.g. `"media_stop"`) are accepted even on platforms
where enigo cannot dispatch them (macOS). The profile is valid; the action silently
no-ops with a `warn!` log. This matches current behaviour for `KeyChord` modifiers that
aren't available on some platforms.

---

## UI changes (task 17.4)

The key-picker in `ActionEditor.svelte` currently renders a flat list. After this epic it
should render keys in four groups:

| Group label | Contents |
|---|---|
| **Standard** | a–z, 0–9, space, return, backspace, tab, escape, and punctuation |
| **Navigation** | arrows, home, end, page up/down, delete, insert, caps lock, num lock, scroll lock, print screen |
| **Function** | f1–f24 |
| **Media / System** | media play/next/prev/stop, volume up/down/mute, brightness down/up, mic mute, eject, pause |

Keys unavailable on the current platform (detected at build time or runtime) are shown
greyed out with a tooltip explaining the platform limitation.

---

## Implementation tasks

| Task | Description |
|---|---|
| **17.1** | This spec. Update `mapping-core-spec.md` §Key enum to match the canonical list above. |
| **17.2** | Fix bugs 1–6 in `key_def.rs` (`VALID_KEYS`) and add the five new system keys. Update `cargo test` (key validation tests). |
| **17.3** | Rewrite `name_to_key` in `pump.rs` with the complete mapping from this spec. Use `#[cfg]`/`cfg!()` for `print_screen`, `scroll_lock`, and all platform-limited system keys. Add `name_to_key` unit tests for all new and fixed arms. |
| **17.4** | Update `ActionEditor.svelte` key-picker UI with grouped layout and platform-greyed keys. |
| **17.5** | Serde round-trip tests for all new key names; document manual verification steps for platform-specific keys. |
