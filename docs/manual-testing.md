# mapxr — Manual Testing Checklist

Test scenarios covering all features of the mapping engine, UI, and CLI tool.
Use the same checkbox convention as the implementation plan: `- [ ]` not tested, `- [x]` confirmed working.

Sections marked _(Epic 8)_ require the `tap-cli` binary to be built first.

---

## 1 — Device Setup & Connection

- [ ] App launches without errors on a clean profile directory
- [ ] Device scan finds at least one TAP device by BLE service UUID
- [ ] Duplicate scan results are deduplicated (same device not listed twice)
- [ ] Assigning a device the `solo` role connects and streams tap events
- [ ] Assigning a device the `left` role connects and identifies as left hand
- [ ] Assigning a device the `right` role connects and identifies as right hand
- [ ] Device registry persists across app restarts — previously paired device reconnects automatically
- [ ] Disconnect button disconnects the device (controller-mode exit packet sent)
- [ ] Device going out of range triggers automatic reconnect with backoff
- [ ] After reconnect, tap events resume correctly
- [ ] App closing sends controller-mode exit to all connected devices before shutdown
- [ ] Warning shown in UI when active profile requires a role not currently connected

---

## 2 — Profile Management

- [ ] Profile list shows all `.json` files in the profiles directory
- [ ] Each profile shows name, kind badge (single/dual), and description
- [ ] Active profile is visually indicated in the list
- [ ] Create new single-hand profile — wizard prompts for name and hand (left/right)
- [ ] Create new dual-hand profile — wizard prompts for name only (no hand field)
- [ ] Activate a profile — it becomes the base layer
- [ ] Delete a profile — requires confirmation before deletion
- [ ] Import a profile via file picker — validates and copies to profiles directory
- [ ] Import a profile via drag-and-drop — same validation as file picker
- [ ] A profile with an invalid key name is rejected on load with a descriptive error
- [ ] A profile with a malformed finger pattern is rejected on load with a descriptive error
- [ ] A profile with a circular alias reference is rejected on load with a descriptive error
- [ ] A profile with overloaded codes but no `overload_strategy` is rejected on load
- [ ] `profile-error` event causes an error message in the UI, not a crash

---

## 3 — Profile Editor

- [ ] Mapping list shows label, trigger summary, action summary, and enabled toggle
- [ ] Enabled toggle sets `"enabled": false` in JSON without deleting the mapping
- [ ] Disabled mapping does not fire when its trigger is activated
- [ ] Mappings can be reordered by drag handle
- [ ] Add mapping opens the editor panel; new mapping appears in list on save
- [ ] Delete mapping shows a 5-second undo toast; mapping is removed if not undone
- [ ] Undo toast restores the deleted mapping in its original position
- [ ] Save button writes the profile to disk and shows a success toast
- [ ] Save error (e.g. write permission denied) shows an error toast, does not crash
- [ ] Navigating away with unsaved changes shows a confirmation dialog
- [ ] Profile settings panel shows all timing fields with labels
- [ ] Changing a timing field in the settings panel takes effect when the profile is saved and reloaded
- [ ] Alias manager: add, edit, and delete named aliases
- [ ] Variable manager: add and delete variables with type and initial value
- [ ] `on_enter` action editor is accessible and saves correctly
- [ ] `on_exit` action editor is accessible and saves correctly

---

## 4 — Finger Pattern Widget

### Single-hand
- [ ] Clicking a circle toggles it between filled (x) and empty (o)
- [ ] Typing `x` or `o` while focused fills the corresponding finger
- [ ] Labels below circles show T/I/M/R/P for right hand (thumb → pinky, left to right)
- [ ] Labels below circles show P/R/M/I/T for left hand (pinky → thumb, left to right)
- [ ] Clicking record mode and tapping the hardware fills the pattern from the device
- [ ] Validation error shown inline when all five circles are empty (all-o is invalid)

### Dual-hand
- [ ] Two groups of five circles with a visual gap between them
- [ ] Left group labelled "Left", right group labelled "Right"
- [ ] Left group reads pinky → thumb (P/R/M/I/T), right group reads thumb → pinky (T/I/M/R/P)
- [ ] Either group can be all-o (idle side of a dual pattern) — no validation error
- [ ] Both groups all-o simultaneously shows validation error
- [ ] Record mode fills the correct hand group based on which device sent the tap

### Display
- [ ] Read-only mode in mapping list summaries shows pattern without click handlers
- [ ] Component renders correctly in light mode
- [ ] Component renders correctly in dark mode

---

## 5 — Trigger Types: Single-hand

### Tap
- [ ] Single tap fires the mapped action exactly once
- [ ] Two different tap codes each fire their own mapped action independently
- [ ] A tap with no mapping in the active layer is consumed silently (passthrough: false)

### Double tap — patient strategy
- [ ] Single tap on an overloaded code fires after `double_tap_window_ms` elapses with no second tap
- [ ] Double tap on an overloaded code fires the `double_tap` action (not the `tap` action)
- [ ] Non-overloaded code fires immediately without waiting for `double_tap_window_ms`

### Double tap — eager strategy
- [ ] Single tap on an overloaded code fires the `tap` action immediately
- [ ] When a second tap arrives within the window, the undo sequence fires, then the `double_tap` action fires
- [ ] Custom `eager_undo_sequence` is used instead of the default backspace when configured
- [ ] Non-overloaded code fires immediately (no undo, no delay)

### Triple tap
- [ ] Three taps within `triple_tap_window_ms` fires the `triple_tap` action
- [ ] Two taps followed by a timeout resolves as double tap (if mapped) or two singles

### Sequence
- [ ] Sequence completes correctly when all steps arrive within `sequence_window_ms`
- [ ] Sequence step timeout: if the next step does not arrive in time, buffered steps flush as individual taps
- [ ] Per-trigger `window_ms` override overrides the profile-level `sequence_window_ms` for that trigger only
- [ ] A tap that does not match the next sequence step aborts the sequence and flushes buffered steps

---

## 6 — Trigger Types: Dual-hand

### Single-device taps within a dual profile
- [ ] Left-hand-only tap (`ooooo` on right side) fires immediately after `combo_window_ms` with no right-hand event
- [ ] Right-hand-only tap (`ooooo` on left side) fires immediately after `combo_window_ms` with no left-hand event
- [ ] Left-only and right-only mappings fire independently in quick succession

### Cross-device chord
- [ ] Both-hand chord arriving within `combo_window_ms` fires the cross-device action
- [ ] Both taps arriving outside `combo_window_ms` each resolve as independent single-device taps
- [ ] Cross-device chord fires the chord action even if each individual pattern is also mapped as a single-device tap

### Dual sequences
- [ ] Cross-device sequence (step 1 on left, step 2 on right) completes correctly
- [ ] Sequence step timeout works the same as in single-hand mode

---

## 7 — Action Types

### key
- [ ] `key` with no modifiers presses and releases the key
- [ ] `key` with `ctrl` sends ctrl+key
- [ ] `key` with `shift` sends shift+key
- [ ] `key` with `alt` sends alt+key
- [ ] `key` with `meta` sends meta+key
- [ ] `key` with multiple modifiers sends all modifiers simultaneously
- [ ] All documented special key names (space, return, escape, tab, backspace, delete, arrow keys, f-keys, media keys) work correctly

### key_chord
- [ ] `key_chord` with two keys presses all keys simultaneously
- [ ] `key_chord` with three or more keys works correctly (e.g. ctrl+alt+delete)

### type_string
- [ ] `type_string` emits the literal text character by character
- [ ] `type_string` handles special characters (quotes, backslash, etc.)
- [ ] `type_string` emits text without applying any currently held modifiers (held modifiers should not affect type_string output)

### macro
- [ ] Macro steps fire in order
- [ ] `delay_ms` on a step causes the specified delay before the next step
- [ ] Events received during macro execution are queued and processed after the macro completes
- [ ] Macro containing `key` and `type_string` steps works correctly

### push_layer / pop_layer / switch_layer
- [ ] `push_layer` permanent: new layer becomes active; original layer remains in stack
- [ ] `push_layer` count: layer is active for exactly N resolved trigger firings, then pops
- [ ] `push_layer` timeout: layer is active for `timeout_ms`, then pops automatically
- [ ] `pop_layer` returns to the layer below in the stack
- [ ] `pop_layer` on the base layer is a no-op and does not crash
- [ ] `switch_layer` replaces the entire stack with the new layer (no layers below it)

### toggle_variable / set_variable
- [ ] `toggle_variable` fires `on_true` action the first time (variable starts false)
- [ ] `toggle_variable` fires `on_false` action the second time
- [ ] `toggle_variable` alternates correctly on subsequent taps
- [ ] `set_variable` explicitly sets the variable to the given value regardless of current state
- [ ] After `set_variable false`, `toggle_variable` fires `on_true` on next tap

### block
- [ ] `block` action consumes the event; nothing is sent to the OS
- [ ] `block` in a passthrough layer stops the walk; lower layers do not fire for that code

### alias
- [ ] `alias` action resolves to the named alias action and fires it correctly
- [ ] Alias referencing another alias resolves one level deep and fires correctly

---

## 8 — hold_modifier (Sticky Modifiers)

### Toggle mode
- [ ] First tap activates the modifier; next `key` action sends the modifier
- [ ] Second tap of the same `hold_modifier` deactivates it; subsequent `key` actions no longer carry the modifier
- [ ] Two different modifier sets (e.g. shift toggle and ctrl toggle) are tracked independently
- [ ] After deactivation, firing the `hold_modifier` again re-activates it

### Count mode
- [ ] `count: 1` — modifier applied to the very next key-dispatching action, then cleared
- [ ] `count: 2` — modifier applied to the next two key-dispatching actions, then cleared
- [ ] `type_string` decrements the count without applying the modifier to the text
- [ ] A whole `macro` counts as one decrement (not per step)

### Timeout mode
- [ ] Modifier is active and applies to key actions within the `timeout_ms` window
- [ ] Modifier is silently removed after the timeout; subsequent key actions do not carry it

### Interactions
- [ ] `hold_modifier` modifier set is unioned with the `key` action's own modifiers — both apply
- [ ] `hold_modifier` state survives a `push_layer` — modifier still active in the new layer
- [ ] `hold_modifier` state survives a `pop_layer` — modifier still active after returning
- [ ] `hold_modifier` inside a macro step is rejected at profile load time with a clear error

---

## 9 — Layer System

### Stack behaviour
- [ ] Base layer is active at app start; `on_enter` fires at startup
- [ ] Pushing a layer makes it the active layer; base layer is still in the stack
- [ ] Stacking two pushed layers — active layer is the topmost; each pop reveals the one below
- [ ] `on_enter` fires each time a layer is pushed onto the stack
- [ ] `on_exit` fires each time a layer is popped from the stack

### Passthrough
- [ ] With `passthrough: true`, an unmatched code falls through to the layer below
- [ ] With `passthrough: false`, an unmatched code is consumed silently
- [ ] Passthrough walk stops at the first layer with a matching binding
- [ ] Passthrough walk reaches the base layer if all layers above have `passthrough: true`

### block in passthrough layers
- [ ] `block` in a passthrough layer consumes the code even though the layer has `passthrough: true`
- [ ] Layers below a `block` do not fire for that code

### Count-mode layer expiry
- [ ] After exactly N taps, the count-mode layer pops — the Nth tap is consumed by the layer before it pops
- [ ] The layer below becomes active immediately after the pop
- [ ] `on_exit` fires when the count-mode layer pops

### Timeout-mode layer expiry
- [ ] The timeout layer pops automatically after `timeout_ms` without any tap
- [ ] `on_exit` fires on timeout expiry
- [ ] Taps arrive on the layer below after expiry

### Pause pattern
- [ ] Pushing a passthrough:false layer with no bindings (except an unpause chord) silences all input
- [ ] The unpause chord fires `pop_layer` and resumes normal operation
- [ ] `on_enter` / `on_exit` fire correctly around the pause

---

## 10 — Profile Variables

- [ ] Variables are initialised to their declared values when a profile is loaded
- [ ] Variable state is visible in the debug/live panel in the UI
- [ ] `toggle_variable` flips a bool variable and fires the correct branch
- [ ] `set_variable` sets the variable to the exact value given
- [ ] Reloading a profile resets all variables to their initial values
- [ ] A variable named `muted` in layer A and a variable named `muted` in layer B are independent — toggling one does not affect the other

---

## 11 — Live Visualiser

- [ ] Finger circles for each connected device update on every tap
- [ ] Last-tap timestamp is shown next to each device's finger display
- [ ] Finger circles animate briefly on tap (highlight fades)
- [ ] Active layer stack breadcrumb shows correct layers (e.g. `base > symbols`)
- [ ] Layer breadcrumb updates immediately on push and pop
- [ ] Current variable values for the active layer are displayed
- [ ] Variable values update immediately when a `toggle_variable` or `set_variable` fires

---

## 12 — Debug Panel

- [ ] Debug mode toggle in header turns debug event emission on and off
- [ ] Debug mode setting persists across app restarts
- [ ] `resolved` events appear in the stream with: finger pattern, matched layer, matched mapping label, action fired, waited_ms vs window_ms timing
- [ ] `unmatched` events appear with finger pattern, layers checked, and reason
- [ ] `combo_timeout` events appear with both patterns, combo window, and actual gap
- [ ] Events appear newest-at-top
- [ ] Event type filter checkboxes (resolved / unmatched / combo_timeout) correctly hide/show event types
- [ ] Pause button stops new events from appearing in the list (they are still emitted by the engine)
- [ ] Resume button causes buffered events to appear
- [ ] Clear button empties the event list
- [ ] Export button downloads the current event stream as a `.jsonl` file
- [ ] Exported `.jsonl` file can be reopened and parsed as valid JSON lines

---

## 13 — Settings Reference

- [ ] `combo_window_ms` — reducing the value causes more cross-device taps to resolve as singles; increasing allows slower cross-device chords
- [ ] `sequence_window_ms` — reducing causes sequences to abort sooner; increasing gives more time between steps
- [ ] `double_tap_window_ms` — affects how quickly a second tap must arrive to register as double tap
- [ ] `triple_tap_window_ms` — affects the full window for three taps to register as triple tap
- [ ] Per-sequence `window_ms` override takes effect for that sequence only without affecting other sequences or global timing

---

## 14 — Profile Validation Edge Cases

- [ ] Dual pattern used in a `kind: "single"` profile is rejected at load time
- [ ] Single pattern used in a `kind: "dual"` profile is rejected at load time
- [ ] Macro step containing another `macro` action is rejected at load time
- [ ] `alias` action referencing a name not in the `aliases` map is rejected at load time
- [ ] `push_layer` referencing a `layer_id` not in the layer registry shows a warning or error
- [ ] `ooooo` as a standalone single trigger (not idle side of a dual) is rejected
- [ ] Finger pattern with characters other than `o`/`x` is rejected
- [ ] Finger pattern with wrong length (not 5 or `5 5`) is rejected
- [ ] `hold_modifier` with empty `modifiers` array is rejected at load time
- [ ] `hold_modifier` with duplicate modifiers is rejected at load time
- [ ] `hold_modifier` with `count: 0` is rejected at load time
- [ ] `hold_modifier` with `timeout_ms: 0` is rejected at load time

---

## 15 — CLI Tool: tap-mapper _(Epic 8)_

### validate
- [ ] `tap-mapper validate <valid-file>` exits 0 and prints `OK: <file>`
- [ ] `tap-mapper validate <invalid-file>` exits 1 and prints each error on its own line to stderr
- [ ] `tap-mapper validate <missing-file>` exits 2 and prints "Error: file not found: <path>"

### normalize
- [ ] `tap-mapper normalize <file-with-integer-codes>` rewrites integer codes to finger-pattern strings in place
- [ ] `tap-mapper normalize <already-clean-file>` is idempotent — reports `0 codes updated`, file unchanged
- [ ] `tap-mapper normalize <file> --dry-run` prints normalised JSON to stdout without writing
- [ ] `tap-mapper normalize <file> --output <path>` writes to the new path, leaves original unchanged
- [ ] After normalisation, `tap-mapper validate` on the output file passes

### migrate
- [ ] `tap-mapper migrate <file>` on a version 1 file prints `Already at latest schema version (1)` and exits 0

### lint
- [ ] `tap-mapper lint <valid-clean-file>` exits 0 with no output
- [ ] `tap-mapper lint <file-with-overloaded-codes-no-strategy>` exits 0 but prints a WARN line
- [ ] `tap-mapper lint <file-with-combo_window_ms-below-30>` prints a WARN about short combo window
- [ ] `tap-mapper lint <file-with-double_tap_window_ms-below-100>` prints a WARN about short double-tap window
- [ ] `tap-mapper lint <file-with-unlabelled-mapping>` prints a WARN with the mapping index
- [ ] `tap-mapper lint <invalid-file>` exits 1 (errors present, not just warnings)
- [ ] All lint output follows `<file>:<LEVEL>: <message>` format

---

## 16 — End-to-End Workflow Scenarios

### Single-hand coding workflow
- [ ] Load a right-hand single profile; thumb tap sends space, pinky sends a letter
- [ ] Overloaded pinky: single tap sends lowercase letter, double tap sends ctrl+letter
- [ ] Push nav layer via ring+pinky chord; arrow keys work; pop returns to base
- [ ] Sequence (two steps) triggers a `type_string` snippet

### Dual-hand workflow
- [ ] Left middle fires `c`, right middle fires `v`; both together fire a cross-device chord
- [ ] Both thumbs chord fires space
- [ ] Left two outer fingers push a symbols overlay with a 2-second timeout; overlay expires automatically
- [ ] Inside symbols overlay: right index fires `$`, right middle fires `@`, unmatched fall through to base
- [ ] `block` in symbols overlay prevents the base-layer binding for both-thumbs from firing
- [ ] Pop symbols overlay via its own pop binding; base layer bindings resume

### Pause workflow
- [ ] All-fingers-left tap pushes the pause layer (passthrough:false, no bindings except unpause)
- [ ] All taps during pause are silently consumed
- [ ] All-fingers-both-hands tap fires `pop_layer` and resumes normal operation
- [ ] `on_enter` and `on_exit` signals fire around the pause

### Sticky modifier workflow
- [ ] Activate shift via `hold_modifier count:1`; next key letter arrives shifted; following letter is unshifted
- [ ] Activate shift toggle; multiple key presses are shifted; second `hold_modifier` tap deactivates shift
- [ ] Shift timeout expires mid-session without crashing; subsequent keys are unshifted

### Variable-driven toggle
- [ ] Toggle variable controls a two-state behaviour (e.g. muted/unmuted)
- [ ] State shown in live visualiser panel
- [ ] Reload profile resets variable to initial value
