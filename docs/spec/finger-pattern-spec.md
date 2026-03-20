---
covers: Epic 6 (finger pattern widget component)
status: Approved and fully implemented
last-updated: 2026-03-19
---

# finger-pattern widget — Epic 6 specification

## Table of contents

1. [Overview](#overview)
2. [Anatomy of a finger pattern](#anatomy-of-a-finger-pattern)
3. [Component API](#component-api)
4. [Rendering modes](#rendering-modes)
5. [Interaction model](#interaction-model)
6. [Record mode](#record-mode)
7. [Validation](#validation)
8. [Theming and light/dark mode](#theming-and-lightdark-mode)
9. [Accessibility](#accessibility)
10. [Integration points](#integration-points)
11. [Out of scope](#out-of-scope)

---

## Overview

Epic 6 replaces `FingerPatternPlaceholder` (the plain text input written in Epic 5) with a
visual `FingerPattern` component. The component renders a row of clickable circles — one per
finger — and allows the user to build a tap pattern by clicking rather than typing a string like
`xoooo`.

The component is used in three distinct contexts:

| Context                                    | Mode        | Notes                                       |
| ------------------------------------------ | ----------- | ------------------------------------------- |
| Trigger editor (mapping list, sequence steps) | Interactive | Clickable; emits `change`                   |
| Mapping list summary / debug panel         | Read-only   | Not clickable; compact display              |
| Trigger editor "record" button             | Record      | Next hardware tap fills the pattern live    |

The component must support both single-hand profiles (5 circles) and dual-hand profiles (two
groups of 5 circles separated by a gap).

---

## Anatomy of a finger pattern

The finger order and read direction follow the existing spec exactly (see
`docs/spec/mapping-core-spec.md` § Finger pattern notation):

- **Right hand** (or `hand: "right"` in a single profile): thumb → pinky, left to right.
- **Left hand** (or `hand: "left"` in a single profile): pinky → thumb, left to right.
- **Dual**: left group on the left (pinky→thumb) and right group on the right (thumb→pinky), with a
  visible gap between them. This mirrors the physical position of two hands held out in front of
  the user.

Finger abbreviations: **T** = thumb, **I** = index, **M** = middle, **R** = ring, **P** = pinky.

Labels appear **below** each circle, centred. They use a small monospace font and are always
shown in the interactive and record modes. In read-only mode, labels are optional and default to
hidden (they increase height and are rarely needed in a compact summary context).

---

## Component API

```typescript
// Props interface for <FingerPattern>
interface FingerPatternProps {
  /** The finger pattern string: "xoooo" (5-char) or "oooox xoooo" (11-char dual). */
  code: string;

  /**
   * The hand orientation for single-hand patterns.
   * - "right" (default): positions 0–4 map to T, I, M, R, P (thumb first)
   * - "left": positions 0–4 map to P, R, M, I, T (pinky first)
   * Ignored when `code` contains a space (dual pattern).
   */
  hand?: "left" | "right";

  /**
   * When true, circles are not interactive and no `onchange` fires.
   * The component renders in a compact, non-focusable form.
   * Default: false.
   */
  readonly?: boolean;

  /**
   * When true, the component shows a "waiting for tap" overlay and the next
   * `tap-event` received from the hardware fills the pattern automatically.
   * Clears automatically after the tap is received.
   * Default: false.
   */
  recording?: boolean;

  /**
   * Show finger labels (T/I/M/R/P) below each circle.
   * Default: true in interactive/record mode, false in read-only mode.
   */
  showLabels?: boolean;

  /**
   * Called with the new pattern string when the user clicks a circle or a
   * hardware tap arrives in record mode.
   * Not called in read-only mode.
   */
  onchange?: (code: string) => void;

  /**
   * Called when record mode auto-completes (hardware tap received).
   * Useful for the parent to turn off recording state.
   */
  onrecorded?: () => void;
}
```

The component does **not** own the `recording` state — the parent controls it. The component
fires `onrecorded` when a hardware tap fills the pattern; the parent is responsible for setting
`recording` back to `false`.

---

## Rendering modes

### Interactive (default)

- Each finger is a circle rendered as `btn btn-circle btn-sm`.
- **Tapped** (`x`): filled with `btn-primary`.
- **Not tapped** (`o`): outlined with `btn-outline`.
- Clicking a circle toggles its state and calls `onchange` with the new pattern string.
- Finger labels appear below each circle in a small `font-mono text-xs` span.
- The "active" state respects the `ring` focus indicator for keyboard navigation.

### Read-only

- Circles rendered as non-interactive `<span>` elements (not `<button>`), sized to match `btn btn-circle btn-sm` dimensions.
- Same fill/outline distinction as interactive, but with `cursor-default` and no hover effects.
- Labels hidden by default (`showLabels` defaults to `false`).

### Record

- Visual: all circles rendered in a neutral/outlined state (the pattern the user is about to
  overwrite is shown dimmed, or cleared).
- A subtle pulsing ring or animated indicator on the component signals "waiting for tap".
  Use a Tailwind `animate-pulse` class on the container.
- Finger labels remain visible so the user knows which fingers correspond to which circles.
- When the `tap-event` fires, the component immediately updates the circles to reflect the
  received pattern and calls `onchange` + `onrecorded`.

---

## Interaction model

### Click to toggle

Clicking a circle flips `o` ↔ `x` for that finger. The component constructs the new pattern
string from the current circle states and calls `onchange`.

### Keyboard navigation

When not in read-only mode, the component must be keyboard-operable:

- Each finger circle is a `<button>` (or has `role="checkbox"` and `tabindex="0"`).
- **Space** or **Enter** on a focused circle toggles it.
- **Arrow keys** (left/right) move focus between circles within the same hand group.
- **Tab** cycles through finger circles and the (optional) record button.

### Dual pattern gap

For dual patterns the two groups are rendered in a flex row with a `gap-4` or similar spacer
between them. The gap is purely visual — focus order continues naturally from the last left-hand
circle to the first right-hand circle.

---

## Record mode

The parent triggers record mode by passing `recording={true}`. While in this state:

1. The **parent** subscribes to the `tap-event` Tauri event (not the component). This keeps the
   component free of Tauri dependencies and makes it fully testable in isolation.
2. On receipt of a `RawTapEvent`, the parent decodes the `tap_code` byte using `tapCodeToPattern`
   and calls the component's `onchange` with the resulting pattern string, then sets
   `recording={false}`.
3. The component fires `onrecorded` when it detects that `recording` has been true and a new
   valid pattern has been provided via `onchange`. This signals the parent that it can clean up
   its event listener.
4. The component's visual state (dimmed/pulsing) is driven purely by the `recording` prop.

**Decoding `tap_code` to a pattern string in the frontend:**

The `tap_code` is a `u8` where bits 0–4 correspond to fingers:

| Bit | Right hand | Left hand |
| --- | ---------- | --------- |
| 0   | Thumb      | Pinky     |
| 1   | Index      | Ring      |
| 2   | Middle     | Middle    |
| 3   | Ring       | Index     |
| 4   | Pinky      | Thumb     |

For a single-hand profile the pattern is always 5 characters. For a dual profile the device role
(`"left"` or `"right"`) determines which 5-character group is populated; the other group stays
`ooooo`.

Add a `tapCodeToPattern(tapCode: number, hand: "left" | "right"): string` utility function in
`src/lib/utils/tapCode.ts`.

---

## Validation

Validation rules mirror the Rust `TapCode` rules exactly:

- Pattern must be exactly 5 chars (single) or `5 space 5` chars (dual).
- Only `o` and `x` characters (case-insensitive; canonical form is lowercase).
- `ooooo` is invalid as a standalone single pattern or as both sides of a dual pattern.

In **interactive mode**, the component must not allow the user to reach an invalid state through
clicking (clicking the last `x` off should be prevented — or the `onchange` should not be called
and the circle should visually spring back). Show an inline error message below the component if
the `code` prop itself is invalid (e.g. loaded from a broken file).

In **read-only mode**, an invalid pattern renders the raw string in red with an error icon instead
of the circles (graceful degradation).

---

## Theming and light/dark mode

- Use daisyUI semantic colour tokens exclusively: `primary`, `base-content`, `base-200`,
  `error`, etc. Do not hard-code hex values or Tailwind palette classes like `bg-blue-500`.
- The tapped/untapped distinction must have sufficient contrast in **both** the light and dark
  daisyUI themes. Verify visually during development by toggling `data-theme` on `<html>`.
- The pulsing record indicator must be visible in both themes.

---

## Accessibility

- The component must have a wrapping `<div role="group" aria-label="Finger pattern">` (or
  `aria-label="Left hand finger pattern"` / `"Right hand finger pattern"` for the dual
  sub-groups).
- Each circle button must have `aria-label="Thumb tapped"` / `"Thumb not tapped"` (etc.)
  so a screen reader can announce the current state.
- `aria-pressed` on each button reflects the tapped (`true`) / not-tapped (`false`) state.
- The record-mode overlay must have `aria-live="polite"` and announce when a tap is received.

---

## Integration points

### Profile editor trigger panel

Replace the `<FingerPatternPlaceholder>` used in:

- `src/routes/profiles/[layer_id]/edit/+page.svelte` — trigger editor for tap/double_tap/triple_tap/sequence steps.

The component file lives at `src/lib/components/FingerPattern.svelte`. Delete
`FingerPatternPlaceholder.svelte` immediately once all usages have been migrated to
`FingerPattern.svelte`.

### Mapping list read-only display

The `TriggerSummary` component currently returns a plain text string. It will continue to do so
for the label; the `FingerPattern` component in read-only mode can optionally be placed alongside
the text summary in the mapping list for a visual preview. **This is optional** — do not block
the Epic 6 tasks on this enhancement; note it as a stretch goal within the epic.

### Live tap visualiser (Epic 7)

The record-mode decode utility (`tapCodeToPattern`) will be reused in Epic 7's live visualiser.
Write it in `src/lib/utils/tapCode.ts` as a pure function so it is importable without pulling in
the full component.

---

## Out of scope

The following items are explicitly deferred:

- Animation of individual fingers on tap-event (animated highlight that fades). Listed as Epic 7
  task 7.5 — implement there.
- High-DPI / touch-screen optimisation (circle sizing on mobile). Out of scope until Epic 10.
- SVG-based hand illustration as background of the circles. Considered and rejected for
  complexity; may be revisited as a stretch goal.
