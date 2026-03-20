# MapXr

**MapXr** is what I always wished the Tap Strap and TapXr to be — a highly customisable, local-first mapping tool that, most importantly, supports **two Tap devices simultaneously**.

![image](/screenshot.png)

MapXr is a free desktop application for creating and managing custom keyboard mappings for [Tap Strap and TapXr](https://www.tapwithus.com/) wearable devices. Tap is a finger-worn or wrist-worn Bluetooth keyboard that detects which fingers you tap and sends those chord codes over BLE. MapXr receives those codes, resolves them against your profiles, and fires keystrokes, macros, or layer switches on your desktop — no internet connection required, no account, no subscription.

The real magic happens when you connect two Tap devices at once. MapXr lets you define dual profiles that combine both hands, enabling single-tap combos across all 10 fingers. One device supports 31 unique single-tap combinations. With two devices, that grows to 1023 — which means you will never need to reach for double-tap and triple-tap mappings.

---

## Installation

Download the latest release for your platform from the [Releases page](https://github.com/halfmonty/mapxr/releases).

| Platform | File to download |
| -------- | ---------------- |
| Windows  | `mapxr_x.y.z_x64-setup.exe` (NSIS installer) or `mapxr_x.y.z_x64_en-US.msi` |
| Linux (Ubuntu / Debian) | `mapxr_x.y.z_amd64.deb` |
| Linux (Fedora / RHEL) | `mapxr_x.y.z_x86_64.rpm` |
| Linux (universal) | `mapxr_x.y.z_amd64.AppImage` |

### Windows note

Windows may show a SmartScreen warning on first launch because the app is not yet widely known. Click **More info → Run anyway** to proceed. This warning disappears as the app accumulates download reputation.

### Linux note

The AppImage is self-contained. Make it executable before running:
```bash
chmod +x mapxr_*.AppImage
./mapxr_*.AppImage
```

---

## First-time setup

1. **Launch MapXr.** On first launch the window opens in the foreground. On subsequent launches the window may be hidden to the tray — click the tray icon to show it.
2. **Go to the Devices page.** Click **Scan** to discover nearby Tap devices (scan lasts ~5 seconds). Assign a role (`solo`, `left`, or `right`) to each device and click **Connect**.
3. **Create a profile.** Go to the **Profiles** page and click **New**. Give it a name and start adding mappings in the editor.
4. **Activate the profile.** Click **Activate** on the Profiles page. Your mappings are now live — you can close the window and the app continues running in the system tray.
5. *(Optional)* **Set up context rules.** Go to **Context Rules** to automatically switch profiles based on which application is in focus.

---

## What is wrong with double and triple taps?
The main issue with double and triple taps comes from overloading an already mapped single tap action. For example, if this tap pattern ●○○○○ is bound to the letter 'a', and a double tap of ●○○○○ is 'v', as is the case in the default single tap map, there are limited ways to determine which letter you actually want to send.

**Eager approach** - The first tap is detected and the letter 'a' is sent, then the second tap is detected, identified as a double tap, 'backspace' is then sent to delete the 'a' and then 'v' is sent.

This is the approach that the Tap implements by default. This behavior prevents using double or triple taps in any sort of mutli-key shortcuts because the initial key being sent will mess up the shortcut.

**Patient Approach** - The software uses a window of time to determine if a double tap has occured that it ensures has elapsed to determine if a tap was single or double.

The downside to this approach is that every tap now takes longer to send because it must wait the double tap timeout before knowing which key to send. This is exacerbated further with triple taps.


#### Two Tap Solution
By having 1023 single taps available between 10 fingers, plus the ability to switch as many `layers` as you want, there is effectively no reason to need double and triple taps. As a result MapXr will determine if your map file doesn't include any single or double taps and will eliminate any unecessary delay making your taps more responsive.

---


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
| `mouse_click`      | Click a mouse button (`left`, `right`, `middle`).                                  |
| `mouse_double_click` | Double-click a mouse button.                                                     |
| `mouse_scroll`     | Scroll in a direction (`up`, `down`, `left`, `right`) by a configurable amount.    |
| `vibrate`          | Send a vibration pattern to the connected Tap device(s).                           |

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

### Context Rules page

The **Context Rules** page lets MapXr automatically switch profiles based on which application is currently in focus.

Each rule has:
- **Pattern** — a substring or glob matched against the focused window's title or application class.
- **Profile** — the profile to activate when the pattern matches.
- **Priority** — rules are evaluated top-to-bottom; the first match wins. Drag rows to reorder.

When a focused window matches a rule, MapXr activates the corresponding profile silently in the background. When the window loses focus and no other rule matches, the previous profile remains active. If no rule matches at all, no automatic switch occurs.

Context switching works on **Linux** (X11 and Wayland) and **Windows**. macOS is not currently supported.

### Settings page

The **Settings** page controls global application behaviour.

| Setting | Description |
| ------- | ----------- |
| Close button behaviour | **Minimise to tray** (default) keeps MapXr running when you close the window. **Exit** quits the app and disconnects devices. |
| Start minimised | Launch directly to the tray without showing the window. |
| Start at login | Register MapXr to launch automatically when you log in. |
| **Notifications** | |
| Device connected | Show an OS notification when a Tap device connects. |
| Device disconnected | Show an OS notification when a Tap device disconnects. |
| Layer switch | Show an OS notification when the active layer changes. |
| **Haptics** | |
| Enable haptics | Master toggle for all vibration feedback. |
| On tap | Short pulse on each resolved tap. |
| On layer switch | Distinct pulse when the active layer changes. |
| On profile switch | Distinct pulse when the active profile changes. |

### System tray

MapXr runs in the system tray so your mappings stay active while the window is hidden.

- **Left-click** the tray icon to show or hide the window.
- The tray **tooltip** shows the active profile name and how many devices are connected.
- The tray **menu** has Show / Hide, the active profile name (greyed out), and Quit.
- Choosing **Quit** from the tray menu disconnects all BLE devices cleanly before exiting.

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

### Releasing a new version

Push a version tag to trigger the unified release workflow, which builds Linux and Windows installers in parallel and publishes them to a GitHub Release automatically:

```bash
git tag v1.0.0
git push origin v1.0.0
```

Tags containing `-` (e.g. `v1.0.0-beta.1`) are published as GitHub pre-releases. All other `v*` tags publish as full releases.

**One-time repository setup required:**

1. Go to **Settings → Actions → General → Workflow permissions** and set to **"Read and write permissions"** so the workflow can create GitHub Releases.
2. Add the updater signing key as a repository secret under **Settings → Secrets → Actions**:
   - `TAURI_SIGNING_PRIVATE_KEY` — the private key generated by `cargo tauri signer generate`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the key password (if set)

### Dev builds (manual, no release)

Use the individual workflows to produce test builds without creating a GitHub Release:

- **Actions → Build Linux → Run workflow** — produces AppImage, deb, rpm
- **Actions → Build Windows → Run workflow** — produces MSI and NSIS exe


---

MapXr is a personal project. [Tap Strap](https://www.tapwithus.com/) is a product of Tap Systems Inc.
