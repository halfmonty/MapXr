# tap-mapper — mapping-core specification

## Table of contents

1. [Overview](#overview)
2. [Finger pattern notation](#finger-pattern-notation)
3. [Profile kinds](#profile-kinds)
4. [Profile structure](#profile-structure)
5. [Trigger types](#trigger-types)
6. [Action types](#action-types)
7. [Layer system](#layer-system)
8. [Profile variables](#profile-variables)
9. [Settings reference](#settings-reference)
10. [Named aliases](#named-aliases)
11. [Debug mode](#debug-mode)
12. [Full annotated examples](#full-annotated-examples)
13. [Engine behaviour rules](#engine-behaviour-rules)
14. [Schema version and migration](#schema-version-and-migration)
15. [Stretch goals (out of scope)](#stretch-goals-out-of-scope)

---

## Overview

The mapping-core crate is the platform-agnostic heart of tap-mapper. It receives raw tap events from
the BLE layer, resolves them against the active profile stack, and returns actions for the platform
layer to execute. It has no knowledge of BLE, Tauri, Android, or OS-level input injection — those
concerns belong to the layers above and below it.

A profile is a single JSON file on disk. Profiles are human-readable and hand-editable. The file
format is designed so that a user who has never seen the spec can read a profile and understand what
it does.

---

## Finger pattern notation

Tap codes are written as strings of `o` (finger not tapped) and `x` (finger tapped).

### Single-hand pattern

Five characters, one per finger. The read direction depends on the `hand` value declared at profile
level:

| `hand` value        | Read direction | Example              |
| ------------------- | -------------- | -------------------- |
| `"right"` (default) | thumb → pinky  | `xoooo` = thumb only |
| `"left"`            | pinky → thumb  | `xoooo` = pinky only |

This convention mirrors how fingers appear when both hands face away — the left hand reads outward
(pinky on the left) and the right hand reads outward (pinky on the right), so both read from the
body's centreline outward.

```
Left hand (pinky→thumb):   x o o o o
                           P R M I T
                           ↑
                           outer edge

Right hand (thumb→pinky):  o o o o x
                                   T I M R P
                                           ↑
                                           outer edge
```

Where: T=thumb, I=index, M=middle, R=ring, P=pinky.

### Two-hand pattern (dual profiles only)

Two five-character groups separated by a single space. The left group is always pinky-to-thumb; the
right group is always thumb-to-pinky. Handedness is implicit from position.

```
"oooox xoooo"
 ↑↑↑↑↑ ↑↑↑↑↑
 PRIMТ ТMIRP
 Left  Right

"oooox xoooo" = both thumbs simultaneously
"ooxoo ooooo" = left middle finger only, right hand idle
"ooooo ooxoo" = left hand idle, right middle finger only
```

`ooooo` on either side means that hand contributes nothing to this trigger. The engine uses the
combo window to determine whether a single-hand event within a dual profile should be resolved
immediately or held pending the other hand.

### Validation rules

- Must be exactly 5 characters (single) or `5 5` characters with one space (dual).
- Characters must be `o` or `x` only (case-insensitive at parse time; canonical form is lowercase).
- A single-hand pattern of `ooooo` is only valid as the idle side of a dual pattern or as an
  explicit `block` action target. It is rejected as a standalone trigger.
- A dual pattern of `ooooo ooooo` is always invalid.

### Legacy integer support

For migration from earlier tooling, the deserializer also accepts a raw `u8` integer (0–31). This
is accepted but the serializer always writes the string form. Running `tap-mapper normalize` on a
profile rewrites all integers to finger-pattern strings.

---

## Profile kinds

Every profile declares a `kind` at the top level. This is the contract between the profile and the
hardware configuration. The engine checks `kind` against the number of connected devices at load
time and warns if they do not match.

| `kind`     | Devices required | Code format                      |
| ---------- | ---------------- | -------------------------------- |
| `"single"` | 1                | 5-char string + top-level `hand` |
| `"dual"`   | 2                | `"ooooo ooooo"` two-part string  |

A single-hand profile will not load on a dual setup without explicit conversion, and vice versa.
This is intentional — the mappings have different semantics.

---

## Profile structure

```jsonc
{
  // ── Required ──────────────────────────────────────────────────
  "version": 1,              // Schema version. Currently always 1.
  "kind": "single",          // "single" | "dual"
  "name": "my-profile",      // Human-readable name shown in UI.
  "layer_id": "base",        // Unique identifier for this layer.
                             // Used by push_layer / pop_layer actions.
                             // Defaults to the filename stem if omitted.

  // ── Kind-specific ─────────────────────────────────────────────
  "hand": "right",           // "left" | "right". Single profiles only.
                             // Omit for dual profiles. Defaults to "right".

  // ── Optional ──────────────────────────────────────────────────
  "description": "...",      // Free-text description shown in UI.
  "passthrough": false,      // If true, unmatched codes fall through to the
                             // next layer down the stack. Defaults to false.

  "settings": { ... },       // Timing and behaviour overrides. See Settings.
  "aliases": { ... },        // Named reusable actions. See Aliases.
  "variables": { ... },      // Named boolean/integer state. See Variables.

  "on_enter": { ... },       // Action fired when this layer becomes active.
  "on_exit":  { ... },       // Action fired when this layer is popped.

  // ── Finger pattern reference comment (recommended) ────────────
  "_pattern_guide": "single right: thumb→pinky | single left: pinky→thumb | dual: 'ooooo ooooo' left(pinky→thumb) right(thumb→pinky)",

  // ── Mappings ──────────────────────────────────────────────────
  "mappings": [ ... ]
}
```

---

## Trigger types

### `tap`

A single simultaneous chord. The engine waits for `combo_window_ms` before resolving to ensure
no cross-device combo or double-tap is pending (when those are configured).

```jsonc
{
  "trigger": {
    "type": "tap",
    "code": "oooox", // Single or dual pattern depending on profile kind.
  },
}
```

### `double_tap`

The same chord twice within `double_tap_window_ms`. If the same code appears in both a `tap` and
a `double_tap` binding, the code is overloaded and the `overload_strategy` setting applies.

```jsonc
{
  "trigger": {
    "type": "double_tap",
    "code": "oooox",
  },
}
```

### `triple_tap`

The same chord three times within `triple_tap_window_ms`.

```jsonc
{
  "trigger": {
    "type": "triple_tap",
    "code": "oooox",
  },
}
```

### `sequence`

An ordered list of chords where each step must arrive within `sequence_window_ms` of the previous
step. Steps can be on different hands in a dual profile. A per-trigger `window_ms` overrides the
profile-level default for this sequence only.

```jsonc
{
  "trigger": {
    "type": "sequence",
    "steps": ["oooox ooooo", "ooooo oooox"],
    "window_ms": 400, // Optional. Overrides sequence_window_ms.
  },
}
```

---

## Action types

### `key`

Press and release a single key with optional modifiers held during the press.

```jsonc
{
  "type": "key",
  "key": "a",
  "modifiers": ["ctrl", "shift"], // Optional. Any of: ctrl, shift, alt, meta.
}
```

Valid key name strings follow a readable convention: `"a"`–`"z"`, `"0"`–`"9"`, `"f1"`–`"f24"`,
`"space"`, `"return"`, `"escape"`, `"tab"`, `"backspace"`, `"delete"`, `"left_arrow"`,
`"right_arrow"`, `"up_arrow"`, `"down_arrow"`, `"home"`, `"end"`, `"page_up"`, `"page_down"`,
`"grave"`, `"minus"`, `"equals"`, `"left_bracket"`, `"right_bracket"`, `"backslash"`,
`"semicolon"`, `"quote"`, `"comma"`, `"period"`, `"slash"`, `"media_play"`, `"media_next"`,
`"media_prev"`, `"volume_up"`, `"volume_down"`, `"volume_mute"`. Unknown key names are rejected
at profile load time with a clear error message.

### `key_chord`

All listed keys pressed simultaneously. Use for combinations that are not expressible as a single
key with modifiers (e.g. `ctrl+alt+delete`).

```jsonc
{
  "type": "key_chord",
  "keys": ["ctrl", "alt", "delete"],
}
```

### `type_string`

Emit a literal string character by character using the platform input API.

```jsonc
{
  "type": "type_string",
  "text": "git commit -m \"\"",
}
```

### `macro`

An ordered list of actions with optional delays between them. `delay_ms` on a step is the wait
_after_ that action fires before the next begins. Macros may not nest (a macro step may not itself
be a `macro`).

```jsonc
{
  "type": "macro",
  "steps": [
    { "action": { "type": "key", "key": "escape" }, "delay_ms": 0 },
    { "action": { "type": "type_string", "text": ":wq" }, "delay_ms": 50 },
    { "action": { "type": "key", "key": "return" }, "delay_ms": 0 },
  ],
}
```

### `push_layer`

Push a new layer onto the stack. Three modes:

- `"permanent"` — layer stays until explicitly popped with `pop_layer`.
- `"count"` — layer pops automatically after `count` resolved trigger firings.
- `"timeout"` — layer pops automatically after `timeout_ms` milliseconds.

```jsonc
{ "type": "push_layer", "layer": "nav",  "mode": "permanent" }
{ "type": "push_layer", "layer": "nav",  "mode": "count",   "count": 3        }
{ "type": "push_layer", "layer": "nav",  "mode": "timeout", "timeout_ms": 2000 }
```

### `pop_layer`

Return to the previous layer on the stack. If the stack is already at the base layer, this is a
no-op (logged as a warning).

```jsonc
{ "type": "pop_layer" }
```

### `switch_layer`

Replace the entire stack with a single layer. Used for permanent profile switches where you do not
want to return to the previous context.

```jsonc
{ "type": "switch_layer", "layer": "gaming" }
```

### `toggle_variable`

Read a named boolean variable and fire one of two child actions depending on its current value.
Flips the variable after firing.

```jsonc
{
  "type": "toggle_variable",
  "variable": "muted",
  "on_true": { "type": "key", "key": "f13" },
  "on_false": { "type": "key", "key": "f14" },
}
```

### `set_variable`

Explicitly set a variable to a value without toggling.

```jsonc
{ "type": "set_variable", "variable": "muted", "value": false }
```

### `block`

Explicitly consume a tap code and fire nothing. Used in passthrough layers to prevent a code from
falling through to a lower layer.

```jsonc
{ "type": "block" }
```

### `alias`

Reference a named action defined in the profile's `aliases` map.

```jsonc
{ "type": "alias", "name": "save" }
```

---

## Layer system

### Stack model

The engine maintains a layer stack. The top of the stack is the active layer. When a tap event
arrives, the engine walks down the stack from top to bottom looking for a matching binding:

1. Check the active (top) layer for a matching trigger.
2. If found, fire the action. Stop.
3. If not found and `passthrough: true`, check the next layer down.
4. If not found and `passthrough: false`, the event is consumed silently. Stop.
5. Repeat until a match is found or the stack is exhausted.

A `block` action at any layer stops the passthrough walk at that layer even if lower layers have
a binding for the same code.

### Layer files

Each layer is a separate JSON file. The engine loads layers by `layer_id`. The file directory
is the profile directory; the engine scans it at startup and builds a registry of available
layers by their `layer_id` values.

### Entry and exit actions

`on_enter` fires when a layer is pushed onto the stack (including at startup for the base layer).
`on_exit` fires when a layer is popped. These are single actions, not arrays — use `macro` if
you need multiple steps.

```jsonc
{
  "layer_id": "gaming",
  "on_enter": { "type": "key", "key": "f13" },
  "on_exit": { "type": "key", "key": "f14" },
}
```

### Pause pattern

A "pause" layer is just a layer with `passthrough: false`, no bindings except one that pops back,
and optionally a visual indicator via `on_enter`/`on_exit`:

```jsonc
{
  "version": 1,
  "kind": "dual",
  "name": "paused",
  "layer_id": "paused",
  "passthrough": false,
  "on_enter": { "type": "key", "key": "f15" },
  "on_exit": { "type": "key", "key": "f15" },
  "mappings": [
    {
      "label": "Unpause",
      "trigger": { "type": "tap", "code": "xxxxx xxxxx" },
      "action": { "type": "pop_layer" },
    },
  ],
}
```

---

## Profile variables

Variables are declared at the top level of a profile with their initial values. They persist for
the lifetime of the engine session (not written to disk). They are reset to their initial values
when the profile is reloaded.

```jsonc
{
  "variables": {
    "muted": false,
    "slow_mode": false,
    "tap_count": 0,
  },
}
```

Currently supported types: `boolean`, `integer`. Variables are referenced by name in
`toggle_variable` and `set_variable` actions. The UI can display current variable state in the
debug panel.

---

## Settings reference

All settings are optional. Profile-level settings override the engine defaults. Per-trigger
`window_ms` (on `sequence` triggers) overrides the profile-level `sequence_window_ms` for that
trigger only.

```jsonc
{
  "settings": {
    // ── Timing ────────────────────────────────────────────────────
    "combo_window_ms": 150,
    // How long to wait for a cross-device chord before resolving
    // pending events as individual taps. (Dual profiles only.)

    "sequence_window_ms": 500,
    // Maximum time between steps in a sequence trigger.

    "double_tap_window_ms": 250,
    // Maximum time between the first and second tap of a double_tap.

    "triple_tap_window_ms": 400,
    // Maximum time from first to third tap of a triple_tap.

    // ── Overload resolution ───────────────────────────────────────
    "overload_strategy": "patient",
    // How to handle a code that appears in both a "tap" and a
    // "double_tap" (or "triple_tap") binding.
    //
    // "patient" — wait double_tap_window_ms before firing anything.
    //   Adds latency to every single-tap on overloaded codes.
    //   No visible artifact. Ideal when the double_tap action is
    //   destructive (e.g. delete, cut).
    //
    // "eager"   — fire the single-tap action immediately. If a double
    //   tap is then detected, send the configured undo sequence and
    //   fire the double-tap action instead.
    //   Zero latency. Produces a brief visible artifact.
    //   Ideal for high-frequency single-tap codes.
    //
    // Applies globally to all overloaded codes in the profile.
    // Codes that are NOT overloaded are never delayed regardless of
    // this setting.

    "eager_undo_sequence": [{ "type": "key", "key": "backspace" }],
    // The action(s) used to undo an eagerly-fired single-tap before
    // firing the double-tap action. Only relevant when
    // overload_strategy is "eager". Defaults to a single backspace.
    // Override if your single-tap action is not a single character
    // (e.g. a snippet that emits multiple characters).
  },
}
```

---

## Named aliases

Aliases define reusable actions at the profile level, referenced by name in mappings. This avoids
repeating the same action definition across many bindings and makes profile maintenance easier.

```jsonc
{
  "aliases": {
    "save": { "type": "key", "key": "s", "modifiers": ["ctrl"] },
    "undo": { "type": "key", "key": "z", "modifiers": ["ctrl"] },
    "redo": { "type": "key", "key": "z", "modifiers": ["ctrl", "shift"] },
    "nav_layer": { "type": "push_layer", "layer": "nav", "mode": "permanent" },
    "go_base": { "type": "switch_layer", "layer": "base" },
  },
}
```

Usage in a mapping:

```jsonc
{
  "label": "Save",
  "trigger": { "type": "tap", "code": "ooxoo ooooo" },
  "action": { "type": "alias", "name": "save" },
}
```

Aliases may reference other aliases one level deep. Circular alias references are rejected at
load time.

---

## Debug mode

When debug mode is enabled (via CLI flag or UI toggle), the engine emits structured timing
metadata alongside every resolved event. This is the primary tool for diagnosing why a combo
failed to fire, why a double-tap is not being detected, or why a sequence keeps timing out.

### Debug event payload

```jsonc
{
  "event_type": "resolved",
  "input": {
    "device": "left",
    "raw_code": 4,
    "pattern": "ooxoo ooooo",
    "received_at_ms": 1712345678901,
  },
  "resolution": {
    "strategy": "patient",
    "waited_ms": 247,
    "outcome": "double_tap_fired",
  },
  "layer_stack": ["symbols", "base"],
  "matched_layer": "symbols",
  "matched_mapping": "Double middle → Ctrl+C",
  "action_fired": { "type": "key", "key": "c", "modifiers": ["ctrl"] },
}
```

```jsonc
{
  "event_type": "unmatched",
  "input": {
    "device": "right",
    "raw_code": 2,
    "pattern": "ooooo oooox",
    "received_at_ms": 1712345679050,
  },
  "resolution": {
    "outcome": "no_binding_found",
    "passthrough_layers_checked": ["symbols", "base"],
  },
  "layer_stack": ["symbols", "base"],
}
```

```jsonc
{
  "event_type": "combo_timeout",
  "pending_events": [
    { "device": "left", "pattern": "oooox ooooo", "received_at_ms": 1712345680100 },
    { "device": "right", "pattern": "ooooo oooox", "received_at_ms": 1712345680390 },
  ],
  "combo_window_ms": 150,
  "actual_gap_ms": 290,
  "outcome": "resolved_as_two_singles",
}
```

The UI debug panel displays these events in a live stream with visual timing bars, finger
pattern rendering, and layer stack state. The stream can be filtered by event type and paused.

---

## Full annotated examples

### Example 1 — single-hand right-handed coding profile

```jsonc
{
  "version": 1,
  "kind": "single",
  "hand": "right",
  "name": "coding-right",
  "layer_id": "coding-base",
  "description": "Single right-hand coding profile with nav overlay",
  "_pattern_guide": "thumb→pinky: x=tapped o=idle e.g. xoooo=thumb oooox=pinky",

  "settings": {
    "double_tap_window_ms": 220,
    "overload_strategy": "patient",
  },

  "aliases": {
    "save": { "type": "key", "key": "s", "modifiers": ["ctrl"] },
    "undo": { "type": "key", "key": "z", "modifiers": ["ctrl"] },
  },

  "variables": {
    "caps_on": false,
  },

  "mappings": [
    // Single taps — basic keys
    {
      "label": "Thumb → Space",
      "trigger": { "type": "tap", "code": "xoooo" },
      "action": { "type": "key", "key": "space" },
    },

    {
      "label": "Index → A",
      "trigger": { "type": "tap", "code": "oooox" },
      "action": { "type": "key", "key": "a" },
    },

    // Double tap — overloaded code
    {
      "label": "Index → A (single)",
      "trigger": { "type": "tap", "code": "oooox" },
      "action": { "type": "key", "key": "a" },
    },

    {
      "label": "Index double → Ctrl+A (select all)",
      "trigger": { "type": "double_tap", "code": "oooox" },
      "action": { "type": "key", "key": "a", "modifiers": ["ctrl"] },
    },

    // Alias usage
    {
      "label": "Thumb+Index → Save",
      "trigger": { "type": "tap", "code": "xooox" },
      "action": { "type": "alias", "name": "save" },
    },

    // Variable toggle
    {
      "label": "Pinky → Toggle caps",
      "trigger": { "type": "tap", "code": "oooox" },
      "action": {
        "type": "toggle_variable",
        "variable": "caps_on",
        "on_true": { "type": "key", "key": "caps_lock" },
        "on_false": { "type": "key", "key": "caps_lock" },
      },
    },

    // Push nav layer for 5 taps
    {
      "label": "Ring+Pinky → Nav layer (5 taps)",
      "trigger": { "type": "tap", "code": "ooxxo" },
      "action": { "type": "push_layer", "layer": "nav", "mode": "count", "count": 5 },
    },

    // Sequence
    {
      "label": "Thumb then index → git status",
      "trigger": {
        "type": "sequence",
        "steps": ["xoooo", "oooox"],
        "window_ms": 400,
      },
      "action": { "type": "type_string", "text": "git status" },
    },
  ],
}
```

### Example 2 — dual-hand base profile with passthrough

```jsonc
{
  "version": 1,
  "kind": "dual",
  "name": "dual-base",
  "layer_id": "dual-base",
  "passthrough": false,
  "_pattern_guide": "dual: 'ooooo ooooo' — left(pinky→thumb) space right(thumb→pinky)",

  "settings": {
    "combo_window_ms": 150,
    "sequence_window_ms": 500,
    "double_tap_window_ms": 250,
    "overload_strategy": "eager",
    "eager_undo_sequence": [{ "type": "key", "key": "backspace" }],
  },

  "aliases": {
    "save": { "type": "key", "key": "s", "modifiers": ["ctrl"] },
    "pause": { "type": "push_layer", "layer": "paused", "mode": "permanent" },
  },

  "on_enter": { "type": "key", "key": "f15" },

  "mappings": [
    // Left hand only
    {
      "label": "Left middle → C",
      "trigger": { "type": "tap", "code": "ooxoo ooooo" },
      "action": { "type": "key", "key": "c" },
    },

    // Right hand only
    {
      "label": "Right middle → V",
      "trigger": { "type": "tap", "code": "ooooo ooxoo" },
      "action": { "type": "key", "key": "v" },
    },

    // Cross-device chord — both thumbs
    {
      "label": "Both thumbs → Space",
      "trigger": { "type": "tap", "code": "oooox xoooo" },
      "action": { "type": "key", "key": "space" },
    },

    // Cross-device chord — both index fingers
    {
      "label": "Both index → Save",
      "trigger": { "type": "tap", "code": "ooooх хoooo" },
      "action": { "type": "alias", "name": "save" },
    },

    // Cross-device sequence
    {
      "label": "Left index then right index → git commit",
      "trigger": {
        "type": "sequence",
        "steps": ["oooox ooooo", "ooooo oooox"],
      },
      "action": { "type": "type_string", "text": "git commit -m \"\"" },
    },

    // Push symbols overlay with timeout
    {
      "label": "Left ring+pinky → Symbols (2s)",
      "trigger": { "type": "tap", "code": "xxooo ooooo" },
      "action": { "type": "push_layer", "layer": "symbols", "mode": "timeout", "timeout_ms": 2000 },
    },

    // Pause
    {
      "label": "All left fingers → Pause",
      "trigger": { "type": "tap", "code": "xxxxx ooooo" },
      "action": { "type": "alias", "name": "pause" },
    },

    // Macro
    {
      "label": "Both middle → Open terminal",
      "trigger": { "type": "tap", "code": "ooxoo ooxoo" },
      "action": {
        "type": "macro",
        "steps": [
          { "action": { "type": "key", "key": "grave", "modifiers": ["ctrl"] }, "delay_ms": 100 },
          { "action": { "type": "type_string", "text": "cd ~/projects" }, "delay_ms": 50 },
          { "action": { "type": "key", "key": "return" }, "delay_ms": 0 },
        ],
      },
    },
  ],
}
```

### Example 3 — symbols overlay with passthrough

```jsonc
{
  "version": 1,
  "kind": "dual",
  "name": "symbols",
  "layer_id": "symbols",
  "passthrough": true,
  "description": "Overrides a handful of keys with symbols. Everything else falls through to dual-base.",

  "mappings": [
    // Override right index → $
    {
      "label": "Right index → $",
      "trigger": { "type": "tap", "code": "ooooo oooox" },
      "action": { "type": "type_string", "text": "$" },
    },

    // Override right middle → @
    {
      "label": "Right middle → @",
      "trigger": { "type": "tap", "code": "ooooo ooxoo" },
      "action": { "type": "type_string", "text": "@" },
    },

    // Block a base layer binding without replacing it
    {
      "label": "Block both thumbs in this layer",
      "trigger": { "type": "tap", "code": "oooox xoooo" },
      "action": { "type": "block" },
    },

    // Return to previous layer
    {
      "label": "Left ring+pinky → Exit symbols",
      "trigger": { "type": "tap", "code": "xxooo ooooo" },
      "action": { "type": "pop_layer" },
    },
  ],
}
```

### Example 4 — pause layer

```jsonc
{
  "version": 1,
  "kind": "dual",
  "name": "paused",
  "layer_id": "paused",
  "passthrough": false,
  "description": "All input consumed silently except the unpause chord.",

  "on_enter": { "type": "key", "key": "f16" },
  "on_exit": { "type": "key", "key": "f16" },

  "mappings": [
    {
      "label": "All fingers both hands → Unpause",
      "trigger": { "type": "tap", "code": "xxxxx xxxxx" },
      "action": { "type": "pop_layer" },
    },
  ],
}
```

---

## Engine behaviour rules

These rules define how the engine resolves ambiguous situations. They are not configurable.

1. **Profile load validation.** The engine validates every profile at load time. Unknown action
   types, invalid key names, malformed finger patterns, circular alias references, and overloaded
   codes without a declared `overload_strategy` all produce a load error with a specific message.
   Profiles with errors are not loaded.

2. **Overload detection.** A code is overloaded if it appears in two or more of `tap`,
   `double_tap`, `triple_tap` within the same layer. The `overload_strategy` setting applies to
   the entire profile. Codes that are not overloaded are never delayed.

3. **Combo window scope.** In a dual profile, the combo window applies whenever there is a pending
   event from one device. If the second device fires within `combo_window_ms`, the engine attempts
   a cross-device match. If not, both events resolve independently as single-device taps.

4. **Sequence timeout.** The sequence window resets after each matched step. A sequence times out
   if the next step does not arrive within `sequence_window_ms`. On timeout, all buffered steps
   are flushed and resolved as individual taps.

5. **Stack underflow.** `pop_layer` on a single-item stack is a no-op and emits a warning.
   `switch_layer` on any stack replaces the entire stack with the new layer.

6. **Variable scope.** Variables are per-layer-file. A variable named `muted` in `dual-base` and
   a variable named `muted` in `symbols` are independent. The engine does not share variable state
   across layers.

7. **Count/timeout pop.** When a `push_layer` with `mode: count` exhausts its count, or a timeout
   expires, `on_exit` fires and the layer is popped. The triggering event that exhausted the count
   is consumed by the layer before it pops — it does not re-fire in the layer below.

8. **`block` and passthrough.** A `block` action terminates the passthrough walk at that layer.
   The event is consumed. Nothing fires.

9. **Macro atomicity.** A macro runs to completion before the engine processes the next incoming
   event. Events received during macro execution are queued and processed afterward.

---

## Schema version and migration

The `"version"` field is an integer. The current version is `1`.

When breaking changes are made to the schema, the version number increments and the engine ships
a migration function that transforms `version N` profiles to `version N+1`. The CLI tool
`tap-mapper migrate <file>` applies all pending migrations in order and rewrites the file.

Non-breaking additions (new optional fields, new action types, new trigger types) do not increment
the version.

---

## Stretch goals (out of scope)

These features are explicitly deferred. The schema is designed to accommodate them without breaking
changes.

### Context-aware automatic layer switching

A higher-level daemon process monitors OS state (active window, focused application) and
instructs the engine to push/pop layers based on configurable conditions. This sits above the
engine's event loop and is implemented as a separate component that calls the same `push_layer` /
`pop_layer` API. No schema changes required — the conditions live in a separate
`context-rules.json` file.

```jsonc
// context-rules.json (future, not part of mapping-core)
{
  "rules": [
    {
      "condition": { "type": "active_app", "matches": "code" },
      "layer": "vscode-overlay",
    },
  ],
}
```

### Raw sensor / gesture triggers

The Tap Strap 2 and Tap XR expose accelerometer and gyro data in raw sensor mode. Gesture
triggers (wrist rotation, hand tilt, air swipe) would be a new trigger family alongside `tap`,
`double_tap`, `triple_tap`, and `sequence`. The trigger type system is open-ended by design —
adding `"type": "gesture"` requires no schema version bump.
