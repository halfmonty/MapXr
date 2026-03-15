# live visualiser and debug panel — Epic 7 specification

## Table of contents

1. [Overview](#overview)
2. [Required infrastructure changes](#required-infrastructure-changes)
3. [7a — Live finger visualiser](#7a--live-finger-visualiser)
4. [7b — Debug panel page](#7b--debug-panel-page)
5. [Component and store changes](#component-and-store-changes)
6. [Out of scope](#out-of-scope)

---

## Overview

Epic 7 adds real-time feedback to the UI: a persistent live visualiser in the sidebar showing
the current finger state of each connected device, and a full-featured debug panel page showing
engine decision events.

The live visualiser (7a) is always visible regardless of which page the user is on. The debug
panel (7b) lives at `/debug` and replaces the stub written in Epic 5.

**No new Tauri commands or Rust engine logic are required** except for the two infrastructure
fixes described in §2.

---

## Required infrastructure changes

These changes must be made before implementing any tasks in 7a or 7b. Both touch existing code
and require care.

### 2.1 — Fix `DebugEvent` serde representation (Rust)

The current `DebugEvent` enum in `crates/mapping-core/src/engine/debug_event.rs` uses the
default serde externally-tagged representation (`{"Resolved": {...}}`). The TypeScript type
already expects an internally-tagged format with `"kind"` as the discriminant. Add serde
attributes to match:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DebugEvent { ... }
```

This produces JSON like `{"kind": "resolved", "pattern": "xoooo", ...}`.

Also add `window_ms: u64` to the `Resolved` variant — this is the engine timing window that
applies to the resolved event (combo window for cross-device, double/triple-tap window for
overloaded codes). Without it, the timing bar in task 7.8 cannot be rendered correctly.

Updated `Resolved` variant:

```rust
Resolved {
    pattern: String,
    device: String,
    layer_stack: Vec<String>,
    matched_layer: String,
    matched_mapping: String,
    action_fired: Action,
    waited_ms: u64,
    /// The timing window (ms) that governed this resolution. Used by the UI
    /// to render a waited_ms / window_ms timing bar.
    window_ms: u64,
},
```

The engine already has the window value at resolution time; it just needs to be threaded through
to the `DebugEvent`. Update tests accordingly.

### 2.2 — Tighten `DebugEvent` TypeScript type

Replace the loose `DebugEvent` interface in `src/lib/types.ts` (which uses an index signature)
with a proper discriminated union. This also adds the `window_ms` field from §2.1:

```typescript
export type DebugEvent =
  | {
      kind: "resolved";
      pattern: string;
      device: string;
      layer_stack: string[];
      matched_layer: string;
      matched_mapping: string;
      action_fired: Action;
      waited_ms: number;
      window_ms: number;
    }
  | {
      kind: "unmatched";
      pattern: string;
      device: string;
      passthrough_layers_checked: string[];
    }
  | {
      kind: "combo_timeout";
      first_pattern: string;
      first_device: string;
      second_pattern: string;
      second_device: string;
      combo_window_ms: number;
      actual_gap_ms: number;
    };
```

Remove the `TODO: task 7.8–7.10` comment from the old type once replaced.

### 2.3 — Extend `debugStore` with per-device tap state

The live visualiser needs to know the last tap code per device role. Extend `DebugStore` in
`src/lib/stores/debug.svelte.ts`:

```typescript
interface DeviceTapState {
  tapCode: number;
  receivedAtMs: number;
  /** True for ~500ms after the tap arrives; parent clears with setTimeout. */
  flash: boolean;
}

// Inside DebugStore class:
lastTapByRole = $state<Record<string, DeviceTapState>>({});

recordTap(payload: TapEventPayload): void {
  this.lastTap = payload;
  this.lastTapByRole = {
    ...this.lastTapByRole,
    [payload.device_id]: {
      tapCode: payload.tap_code,
      receivedAtMs: payload.received_at_ms,
      flash: true,
    },
  };
  // Clear flash after animation completes.
  setTimeout(() => {
    this.lastTapByRole = {
      ...this.lastTapByRole,
      [payload.device_id]: { ...this.lastTapByRole[payload.device_id]!, flash: false },
    };
  }, 500);
}
```

---

## 7a — Live finger visualiser

### Task 7.1 — Persistent per-device tap display

Add a "Live" section to the bottom of the sidebar in `src/routes/+layout.svelte`, below the nav
links and separated by a divider. The section is only rendered when at least one device is
connected (`deviceStore.connected.length > 0`).

For each connected device, show:
- The device role as a small label (`"solo"`, `"left"`, `"right"`)
- A `<FingerPattern>` in **read-only** mode, displaying `lastTapByRole[device.role]?.tapCode`
  decoded via `tapCodeToPattern`, with `flash` driven by `lastTapByRole[device.role]?.flash`
- If no tap has been received for this device yet, display an all-`o` pattern (`"ooooo"`)

**Hand orientation**: right-hand device → `hand="right"`; left-hand device → `hand="left"`;
solo device → `hand="right"` (default).

The section header "Live" uses a small uppercase label style consistent with the nav section.

### Task 7.2 — Timestamp and tap code

Below each `<FingerPattern>` in the sidebar live section, show:
- The raw tap code as a hex value (e.g., `0x01`) in `font-mono text-xs text-base-content/50`
- The relative time since last tap, e.g., "2s ago", updated every second via a `setInterval`
  in the layout component

If no tap has been received yet, show `"—"` for both fields.

**Relative time implementation**: a simple helper:
```typescript
function relativeTime(ms: number): string {
  const elapsed = Math.floor((Date.now() - ms) / 1000);
  if (elapsed < 5) return "just now";
  if (elapsed < 60) return `${elapsed}s ago`;
  return `${Math.floor(elapsed / 60)}m ago`;
}
```
Drive updates with a `$effect` + `setInterval` that ticks a `$state<number>` counter every
second to force re-evaluation.

### Task 7.3 — Layer stack breadcrumb

The layer breadcrumb already exists in the footer status bar (implemented in Epic 5). For task
7.3, **move it** from the footer into the sidebar — specifically into a "State" section between
the nav links and the live section.

Display:
- Label: "Layer" in the same small label style
- Stack: `base › symbols › nav` with `›` separators; the active (rightmost) layer in
  `text-base-content font-semibold`, preceding layers in `text-base-content/50`
- If the stack is empty: `"none"` in italic

Remove the layer display from the footer (the footer then shows only connected devices).

### Task 7.4 — Variable values

Below the layer breadcrumb in the sidebar, show the current variable values from
`engineStore.variables`. Only render this section if the variables object is non-empty.

Display as a compact list:
- Variable name in `font-mono text-xs`
- Value: boolean variables shown as a `badge badge-xs` (green for true, neutral for false);
  integer variables shown as the number

### Task 7.5 — Tap flash animation

Add a `flash?: boolean` prop to `<FingerPattern>` in
`src/lib/components/FingerPattern.svelte`.

When `flash` is `true`, circles where `char === 'x'` get an additional CSS animation class.
Define the keyframe in `src/app.css`:

```css
@keyframes tap-flash {
  0%   { transform: scale(1.25); }
  100% { transform: scale(1); }
}
```

Apply it to the tapped circle elements:
- In read-only mode: add `[animation:tap-flash_0.45s_ease-out]` to the `<span>` when
  `flash && char === 'x'`
- In interactive mode: add the same class to the `<button>` when `flash && char === 'x'`

The animation is one-shot (no `infinite`). The `FingerPattern` component does not manage the
`flash` lifetime — the parent clears it via `debugStore` after 500ms (§2.3).

---

## 7b — Debug panel page

The `/debug` page replaces the Epic 5 stub. It is a full-width page within the existing
sidebar layout.

### Task 7.6 — Debug mode toggle with persistence

At the top of the `/debug` page, add a `<toggle>` (daisyUI) labelled "Debug mode". When
toggled:
1. Call `setDebugMode(enabled)`.
2. Update `engineStore.debugMode`.
3. Persist to `localStorage` under key `"mapxr.debugMode"`.

On app init (in `+layout.svelte`'s `onMount`, after `engineStore.init()`), read the stored
value and apply it if different from the backend state:

```typescript
const stored = localStorage.getItem("mapxr.debugMode");
if (stored !== null) {
  const enabled = stored === "true";
  if (enabled !== engineStore.debugMode) {
    await setDebugMode(enabled);
    engineStore.debugMode = enabled;
  }
}
```

When debug mode is off, the event stream is empty (no `debug-event` events arrive from
Rust). Display a hint: _"Enable debug mode to start recording engine events."_

### Task 7.7 — Scrolling event stream

The main content area of `/debug` is a scrolling list of `DebugEvent` entries from
`debugStore.debugEvents` (newest at top, max 200 events per the existing `MAX_DEBUG_EVENTS`
constant).

Each event is rendered as a compact card with a left-border colour indicating event type:
- `resolved` → `border-l-4 border-success`
- `unmatched` → `border-l-4 border-warning`
- `combo_timeout` → `border-l-4 border-error`

Scroll container: `overflow-y-auto` with a fixed `max-h` that fills the remaining viewport
height. Use `flex-1 min-h-0` on the container so it respects the layout.

When paused (task 7.12), new events still accumulate in the store but the rendered list is
frozen at the snapshot taken when pause was clicked.

### Task 7.8 — `resolved` event card

Each `resolved` event card shows:

```
[read-only FingerPattern]  base › symbols  matched "thumb tap" in symbols
                           Key ctrl+c                          45ms / 150ms ██████░░░░
```

Specifically:
- A `<FingerPattern readonly hand={...}>` for the pattern (5-char single or 11-char dual)
- Layer stack at resolution time as a breadcrumb (small, muted)
- Matched mapping label and layer: `matched "thumb tap" in symbols`
- Action summary via `<ActionSummary>` component
- Timing bar: a `<progress>` element (or Tailwind width utility on a `<div>`) where
  width = `min(waited_ms / window_ms, 1) * 100%`, with `waited_ms` and `window_ms` shown
  as text alongside (e.g., `45ms / 150ms`)
- Device badge: small `badge` showing the device role (`"solo"`, `"left"`, `"right"`)

### Task 7.9 — `unmatched` event card

Each `unmatched` event card shows:
- A `<FingerPattern readonly>` for the pattern
- Device badge
- Label: "No match" in warning colour
- Passthrough layers checked as a comma-separated list (e.g., `Checked: symbols, base`)

### Task 7.10 — `combo_timeout` event card

Each `combo_timeout` event card shows:
- Two `<FingerPattern readonly>` side by side — one per device
- Device badges for each pattern
- Gap vs window: e.g., `Gap: 290ms (window: 150ms)` with the gap in error colour when it
  exceeds the window (it always will for this event type, but the colour makes it scannable)
- A simple two-segment bar: `window_ms` portion in `bg-success`, excess portion in `bg-error`

### Task 7.11 — Event type filter

Above the event stream, show three toggle buttons (or checkboxes) for:
- ✓ Resolved
- ✓ Unmatched
- ✓ Combo timeout

All checked by default. Filtering is applied to the rendered list only — the underlying store
buffer is not affected. Use a `$state<Set<string>>` of enabled kinds and filter the events
array in a `$derived`.

### Task 7.12 — Pause / resume

A `Pause` / `Resume` button in the toolbar above the event stream. When paused:
- Capture a `$state` snapshot of `debugStore.debugEvents` at the moment pause is clicked.
- Render the snapshot instead of the live store array.
- Show a `badge badge-warning` indicator: `"Paused — N events buffered"` where N is the
  count accumulated since pausing.
- On Resume: merge buffered events (prepend them) and resume live rendering.

### Task 7.13 — Clear button

A `Clear` button in the toolbar calls `debugStore.clear()`. If currently paused, also clears
the paused snapshot.

### Task 7.14 — Export as `.jsonl`

An `Export` button serialises `debugStore.debugEvents` (the live buffer, not the filtered view)
as one JSON object per line (JSONL format) and triggers a browser download:

```typescript
function exportEvents() {
  const lines = debugStore.debugEvents.map((e) => JSON.stringify(e)).join("\n");
  const blob = new Blob([lines], { type: "application/jsonl" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `mapxr-debug-${new Date().toISOString().slice(0, 19).replace(/:/g, "-")}.jsonl`;
  a.click();
  URL.revokeObjectURL(url);
}
```

---

## Component and store changes summary

| File | Change |
|---|---|
| `crates/mapping-core/src/engine/debug_event.rs` | Add `#[serde(tag="kind", rename_all="snake_case")]`; add `window_ms` to `Resolved` |
| `src/lib/types.ts` | Replace loose `DebugEvent` with tight discriminated union |
| `src/lib/stores/debug.svelte.ts` | Add `lastTapByRole` with per-device flash state |
| `src/lib/components/FingerPattern.svelte` | Add `flash?: boolean` prop + `@keyframes` usage |
| `src/app.css` | Add `@keyframes tap-flash` definition |
| `src/routes/+layout.svelte` | Add sidebar "State" (layer/variables) and "Live" (per-device) sections; remove layer from footer; apply debug mode persistence on init |
| `src/routes/debug/+page.svelte` | Full debug panel replacing Epic 5 stub |

---

## Out of scope

- **Persistent debug log across sessions**: the rolling buffer is in-memory only. Saving to disk
  requires a Tauri command; deferred.
- **Syntax highlighting for `action_fired` JSON**: render via `<ActionSummary>` only.
- **Timestamps per event**: `DebugEvent` does not carry a wall-clock timestamp (only `waited_ms`
  relative to resolution). Adding one would require a Rust struct change; deferred to a future
  improvement.
- **Event diffing / deduplication**: not required for initial implementation.
