# mapxr

**mapxr** is a desktop application for creating and managing custom keyboard mappings for [Tap Strap](https://www.tapwithus.com/) wearable devices. Tap Straps are finger-worn Bluetooth keyboards that detect which fingers you tap and send those chord codes over BLE. mapxr receives those codes, resolves them against your profiles, and fires keystrokes, macros, or layer switches on your desktop.

Built with [Tauri](https://tauri.app/) (Rust backend) and [Svelte 5](https://svelte.dev/) (frontend).

---

## Table of contents

1. [Features](#features)
2. [Requirements](#requirements)
3. [Building and running](#building-and-running)
4. [Concepts](#concepts)
   - [Finger pattern notation](#finger-pattern-notation)
   - [Profiles and layers](#profiles-and-layers)
   - [Triggers](#triggers)
   - [Actions](#actions)
   - [Variables](#variables)
   - [Settings and timing](#settings-and-timing)
5. [Using the application](#using-the-application)
   - [Devices page](#devices-page)
   - [Profiles page](#profiles-page)
   - [Profile editor](#profile-editor)
   - [Debug panel](#debug-panel)
6. [Profile file format](#profile-file-format)
7. [Example profiles](#example-profiles)

---

## Features

- **BLE device management** — scan for nearby Tap Strap devices, connect them, assign roles (solo, left, right), and auto-reconnect on subsequent launches.
- **Profile system** — JSON-based profiles stored as plain files. Human-readable and hand-editable. Live-reloaded when files change.
- **Layer stack** — push temporary layers onto a stack and pop them off. Layers can be permanent, count-limited, or timeout-based. Passthrough lets unmatched codes fall through to lower layers.
- **Rich trigger types** — single tap, double tap, triple tap, two-hand combos (dual profiles), and multi-step sequences.
- **Rich action types** — individual keys with modifiers, key chords, typed strings, multi-step macros with delays, layer control, and named boolean/integer variables.
- **Visual finger pattern editor** — click or keyboard-navigate circles to compose tap patterns. Supports single-hand and two-hand (dual) profiles.
- **Live sidebar** — shows the active layer stack, variable values, and a per-device live finger visualiser that flashes when a tap arrives.
- **Debug panel** — real-time stream of resolved, unmatched, and combo-timeout events with visual timing bars, filtering, pause/resume, and JSONL export.

---

## Requirements

- Rust toolchain (1.82 or later)
- Node.js 20+ and npm
- A Tap Strap 2 device (or compatible)
- Linux, macOS, or Windows

---

## Building and running

```bash
# Install frontend dependencies
npm install

# Run in development mode (hot-reload frontend + Rust rebuild on change)
npm run tauri dev

# Build a production binary (Linux / macOS)
npm run tauri build
```

Profile files are stored in the `profiles/` directory next to the application binary. On a development build they are read from `profiles/` at the project root.

### Building for Windows from Linux

Tauri's bundler must run on Windows to produce a proper `.msi` / NSIS installer. Use the included GitHub Actions workflow:

1. Push the repository to GitHub (the `.github/workflows/build-windows.yml` file must be on the branch).
2. Go to **Actions → Build Windows → Run workflow** for a manual build, or push a tag matching `v*` (e.g. `v0.2.0`) to trigger automatically.
3. Download the `mapxr-windows` artifact when the run completes. It contains the `.msi` installer and the NSIS `.exe`.

> **Note on `GITHUB_TOKEN`:** this token is injected automatically by GitHub — no setup required.
> Your repository must have **Settings → Actions → General → Workflow permissions** set to
> **"Read and write permissions"** for the action to attach artifacts to a GitHub Release.

---

## Concepts

### Finger pattern notation

Tap codes are written as strings of `o` (not tapped) and `x` (tapped), one character per finger.

**Single-hand profiles** use a 5-character string. The read direction depends on the `hand` declared in the profile:

| Hand    | Direction     | `xoooo` means |
| ------- | ------------- | -------------- |
| `right` | thumb → pinky | thumb only     |
| `left`  | pinky → thumb | pinky only     |

```
Left:   x o o o o   (P R M I T — reading outward from body centre)
Right:  o o o o x   (T I M R P — reading outward from body centre)
```

Where T=thumb, I=index, M=middle, R=ring, P=pinky.

**Dual profiles** use two 5-character groups separated by a space. The left group is always pinky-to-thumb; the right is always thumb-to-pinky, regardless of the `hand` field.

```
"oooox xoooo"  →  both thumbs simultaneously
"ooxoo ooooo"  →  left middle finger only
"ooooo ooxoo"  →  right middle finger only
```

### Profiles and layers

A **profile** is a JSON file that declares a set of mappings. Each profile has a unique `layer_id`.

The engine maintains a **layer stack**. The top of the stack is the active layer. When a tap arrives:

1. The engine checks the active layer for a matching trigger.
2. If found, the action fires and the event is consumed.
3. If not found and `passthrough: true`, the next layer down is checked.
4. If not found and `passthrough: false`, the event is silently consumed.

Layers can be pushed and popped programmatically with `push_layer` and `pop_layer` actions, allowing you to build modal or context-sensitive input systems — for example, a nav layer active only while you're holding a specific chord, or a "symbols" layer that pops after three uses.

### Triggers

| Type          | Description                                                                  |
| ------------- | ---------------------------------------------------------------------------- |
| `tap`         | A single chord. The engine waits briefly for a potential combo or double-tap before resolving. |
| `double_tap`  | The same chord twice within `double_tap_window_ms`.                          |
| `triple_tap`  | The same chord three times within `triple_tap_window_ms`.                    |
| `sequence`    | An ordered list of chords where each step must arrive within `sequence_window_ms` of the previous. |

When a code appears in both a `tap` and a `double_tap` binding (an **overloaded** code), the `overload_strategy` setting controls behaviour:

- **`patient`** — wait the full `double_tap_window_ms` before firing the single-tap action. No visible artifact; adds latency to every single tap on that code.
- **`eager`** — fire the single-tap action immediately. If a double-tap is then detected, send the `eager_undo_sequence` (default: backspace) and fire the double-tap action instead. Zero latency; produces a brief visible artifact.

### Actions

| Type               | Description                                                                        |
| ------------------ | ---------------------------------------------------------------------------------- |
| `key`              | Press and release a key, optionally with modifiers (`ctrl`, `shift`, `alt`, `meta`). |
| `key_chord`        | Press multiple keys simultaneously (e.g. `ctrl+alt+delete`).                      |
| `type_string`      | Type a literal string character by character.                                      |
| `macro`            | An ordered list of actions with optional `delay_ms` between steps.                |
| `push_layer`       | Push a layer onto the stack. Modes: `permanent`, `count` (auto-pop after N fires), `timeout` (auto-pop after N ms). |
| `pop_layer`        | Return to the previous layer on the stack.                                         |
| `switch_layer`     | Replace the entire stack with a single layer (no return path).                     |
| `toggle_variable`  | Read a boolean variable, fire one of two child actions, then flip the variable.    |
| `set_variable`     | Explicitly set a variable to a value.                                              |
| `block`            | Consume a tap code and fire nothing. Useful in passthrough layers to suppress specific codes. |
| `alias`            | Reference a named action defined in the profile's `aliases` map.                  |

### Variables

Variables are named boolean or integer values declared at the top of a profile. They persist for the lifetime of the engine session (not written to disk) and reset when the profile is reloaded.

```jsonc
"variables": {
  "muted": false,
  "mode": 0
}
```

Use `toggle_variable` to flip a boolean and conditionally fire one of two actions — for example, mute/unmute with a single tap code:

```jsonc
{
  "type": "toggle_variable",
  "variable": "muted",
  "on_true":  { "type": "key", "key": "f13" },
  "on_false": { "type": "key", "key": "f14" }
}
```

Variable values are visible in the sidebar while any profile with variables is active.

### Settings and timing

All settings are optional and have sensible defaults:

| Setting                | Default    | Description                                                    |
| ---------------------- | ---------- | -------------------------------------------------------------- |
| `combo_window_ms`      | 150        | Window for cross-device chords (dual profiles only).           |
| `double_tap_window_ms` | 250        | Window between first and second tap of a double_tap.           |
| `triple_tap_window_ms` | 400        | Window from first to third tap of a triple_tap.                |
| `sequence_window_ms`   | 500        | Maximum time between consecutive steps in a sequence trigger.  |
| `overload_strategy`    | `patient`  | `"patient"` or `"eager"` — how overloaded codes are resolved.  |
| `eager_undo_sequence`  | backspace  | Actions used to undo an eagerly-fired single-tap.              |

Per-trigger `window_ms` on a `sequence` trigger overrides `sequence_window_ms` for that trigger only.

---

## Using the application

### Devices page

The **Devices** page manages your Tap Strap connections.

1. Click **Scan** to discover nearby devices. The scan runs for 5 seconds and lists devices sorted by signal strength.
2. Assign a **role** to each device:
   - `solo` — single Tap Strap setup
   - `left` / `right` — dual Tap Strap setup
3. Click **Connect**. The app saves the role → address mapping to `devices.json` and reconnects automatically on future launches.
4. To disconnect a device, click **Disconnect** next to its role.

Connected devices are shown in the footer bar and in the sidebar Live section, which visualises incoming taps in real time.

### Profiles page

The **Profiles** page lists all profiles found in the profiles directory.

- Click a profile name to view or edit it.
- Click **New** to create a blank profile (opens the editor).
- Click **Activate** to load a profile as the base layer. The activated profile drives all tap resolution until you switch to another or restart.

If a profile file fails to load (malformed JSON, unknown key name, etc.) a warning banner appears with the filename and error message.

### Profile editor

The profile editor lets you create and modify mappings without editing JSON directly.

**Profile-level fields:**

| Field          | Description                                                                         |
| -------------- | ----------------------------------------------------------------------------------- |
| Name           | Display name shown in the profiles list.                                            |
| Layer ID       | Unique identifier used by `push_layer` / `switch_layer` actions.                   |
| Kind           | `single` (one Tap Strap) or `dual` (two Tap Straps).                               |
| Hand           | For single profiles: `left` or `right`. Sets the read direction of all patterns.   |
| Description    | Optional free-text description.                                                     |
| Passthrough    | If enabled, unmatched codes fall through to the layer below on the stack.           |

**Mappings:**

Each mapping has:
- **Label** — a human-readable name for this mapping (shown in the debug panel).
- **Trigger** — click the finger pattern widget to record or edit the tap chord. Use the type selector to choose tap / double tap / triple tap / sequence.
- **Action** — the action to fire. Use the action type selector and fill in the fields.

**Using the finger pattern widget:**

- Click a finger circle to toggle it on (filled) or off (empty).
- Tab to focus a circle, then press Space or Enter to toggle, or press `x` / `o` to set directly.
- Arrow keys move focus between fingers.
- In dual profiles, both the left and right hand patterns are shown side by side.
- At least one finger must be active — the widget prevents toggling the last active finger off.

**Saving:** click **Save** to write the profile to disk. The profile is immediately available for activation.

### Debug panel

The **Debug** page provides a live view of the engine's event stream.

**Enabling debug mode:** toggle the **Debug mode** switch at the top. When disabled, the engine fires actions but emits no debug events. The setting persists across restarts.

**Event types:**

| Badge           | Colour | Meaning                                                                         |
| --------------- | ------ | ------------------------------------------------------------------------------- |
| `resolved`      | green  | A tap was matched to a mapping and the action fired.                            |
| `no match`      | yellow | A tap arrived but no binding was found in any checked layer.                    |
| `combo timeout` | red    | Two single-hand taps arrived too far apart to form a cross-device chord.        |

**Resolved events** show:
- The finger pattern that was received (rendered as circles).
- Which device sent it (role badge).
- The current layer stack at the time of resolution.
- Which layer and which mapping label matched.
- The action that fired.
- A timing bar showing how long the engine waited before resolving vs. the configured window.

**Unmatched events** show the pattern, the device, and which layers were checked.

**Combo timeout events** show both patterns side by side with a bar comparing the actual gap to the combo window.

**Toolbar controls:**

| Control  | Description                                                                        |
| -------- | ---------------------------------------------------------------------------------- |
| Filter   | Toggle which event types are shown (Resolved / Unmatched / Combo timeout).         |
| Pause    | Freeze the displayed stream. New events are counted; a banner shows the buffer.    |
| Resume   | Unfreeze. The buffer is discarded and the live stream resumes.                     |
| Clear    | Remove all events from the in-memory store.                                        |
| Export   | Download all events as a `.jsonl` file for offline analysis.                       |

**Sidebar live view** (visible when at least one device is connected):

The sidebar shows a per-device finger visualiser that updates in real time as taps arrive. Each tapped finger briefly scales up to indicate the tap. Below the visualiser the raw hex tap code and a relative timestamp ("just now", "3s ago", etc.) are shown.

---

## Profile file format

Profiles are JSON files in the `profiles/` directory. The filename stem is used as a fallback `layer_id` if the field is omitted.

```jsonc
{
  "version": 1,
  "kind": "single",
  "hand": "right",
  "name": "My Profile",
  "layer_id": "my-profile",
  "description": "An example single-hand profile.",
  "passthrough": false,

  "settings": {
    "double_tap_window_ms": 250,
    "overload_strategy": "patient"
  },

  "aliases": {
    "save": { "type": "key", "key": "s", "modifiers": ["ctrl"] }
  },

  "variables": {
    "muted": false
  },

  "on_enter": { "type": "key", "key": "f13" },
  "on_exit":  { "type": "key", "key": "f14" },

  "mappings": [
    {
      "label": "Thumb — Space",
      "trigger": { "type": "tap", "code": "xoooo" },
      "action":  { "type": "key", "key": "space" }
    },
    {
      "label": "Index — E",
      "trigger": { "type": "tap", "code": "oxooo" },
      "action":  { "type": "key", "key": "e" }
    },
    {
      "label": "Double thumb — Enter",
      "trigger": { "type": "double_tap", "code": "xoooo" },
      "action":  { "type": "key", "key": "return" }
    },
    {
      "label": "Save",
      "trigger": { "type": "tap", "code": "ooxoo" },
      "action":  { "type": "alias", "name": "save" }
    },
    {
      "label": "Toggle mute",
      "trigger": { "type": "tap", "code": "oooox" },
      "action": {
        "type": "toggle_variable",
        "variable": "muted",
        "on_true":  { "type": "key", "key": "f13" },
        "on_false": { "type": "key", "key": "f14" }
      }
    },
    {
      "label": "Push nav layer",
      "trigger": { "type": "tap", "code": "xxxxx" },
      "action":  { "type": "push_layer", "layer": "nav", "mode": "permanent" }
    }
  ]
}
```

---

## Example profiles

### Single-hand coding profile

A right-hand profile where common letter codes map to single taps, the all-five chord opens a nav layer, and double-thumb fires Enter. The nav layer has `passthrough: false` so only its own bindings are active, and a single chord pops back to base.

### Dual-hand media controls

Both thumbs simultaneously = play/pause. Left pinky = previous track, right pinky = next track. Both pinkies = mute toggle (using `toggle_variable`). Cross-device chords use a `combo_window_ms` of 150 ms.

### Modal nav layer

A separate profile pushed on top of the base layer by a dedicated chord. All tap codes map to arrow keys, home/end, and page up/down. A "back" chord fires `pop_layer` to return to the previous context. Using `mode: "count"` on the `push_layer` action lets the layer pop itself after a fixed number of key presses.

### Sequence triggers

A sequence trigger requires multiple chords in order within a time window, useful for rare commands that should not conflict with single-tap bindings:

```jsonc
{
  "label": "Quit application",
  "trigger": {
    "type": "sequence",
    "steps": ["xoooo", "xoooo", "xoooo"],
    "window_ms": 600
  },
  "action": { "type": "key", "key": "q", "modifiers": ["ctrl"] }
}
```

---

mapxr is a personal project. [Tap Strap](https://www.tapwithus.com/) is a product of Tap Systems Inc.
