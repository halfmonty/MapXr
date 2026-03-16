# Spec: `hold_modifier` Action — Sticky Modifiers for mapxr

> **Status: APPROVED**

---

## 1. Motivation

Today the only way to send `Shift+A` is to define an explicit mapping whose action is
`Key { key: "a", modifiers: ["shift"] }`. For a full alphabet that means 26 separate
mappings per modifier key — 104 entries just for Shift/Ctrl/Alt/Meta over the letters.

The `hold_modifier` action introduces **sticky modifier** behaviour: one tap activates a
modifier; subsequent taps automatically inherit it, exactly as the OS "sticky keys"
accessibility feature works. The user maps one finger per modifier (left hand, say) and
retains the comfortable per-letter bindings they already know on the right hand.

### Example

| Tap code      | Action                                            |
|---------------|---------------------------------------------------|
| `oooox xoooo` | `hold_modifier { modifiers: ["shift"], mode: { "count": 1 } }` |
| `ooooo xoooo` | `Key { key: "a" }`                                |

Sequence: tap `oooox xoooo`, then `ooooo xoooo` → OS receives **Shift+A**. The sticky
modifier is consumed after one key event, so the next tap of `ooooo xoooo` produces
plain `a`.

---

## 2. New Action Variant: `hold_modifier`

### 2.1 JSON schema

```json
{ "type": "hold_modifier", "modifiers": ["shift"],          "mode": "toggle"          }
{ "type": "hold_modifier", "modifiers": ["ctrl"],           "mode": "count",     "count": 1      }
{ "type": "hold_modifier", "modifiers": ["alt"],            "mode": "timeout",   "timeout_ms": 2000 }
{ "type": "hold_modifier", "modifiers": ["ctrl", "shift"],  "mode": "toggle"          }
```

Serialisation mirrors `PushLayerMode` exactly: `mode` is a `#[serde(tag = "mode")]`
enum flattened into the parent object (no nesting).

### 2.2 Fields

| Field       | Type              | Required | Constraints                       |
|-------------|-------------------|----------|-----------------------------------|
| `type`      | `"hold_modifier"` | yes      | fixed tag                         |
| `modifiers` | `Vec<Modifier>`   | yes      | non-empty; no duplicates; subset of `Ctrl/Shift/Alt/Meta` |
| `mode`      | `HoldModifierMode`| yes      | see §2.3                          |

### 2.3 `HoldModifierMode` enum

Serialised with `#[serde(tag = "mode", rename_all = "snake_case")]`, flattened.

```
Toggle
Count   { count: u32 }           — count ≥ 1
Timeout { timeout_ms: u64 }      — timeout_ms ≥ 1
```

#### Toggle
- First dispatch of `hold_modifier { modifiers: M, mode: toggle }`: activates M.
- Subsequent dispatch of the **same** `hold_modifier` action (same M set,
  order-independent): deactivates M.
- "Same M set" is an unordered comparison (e.g. `["ctrl","shift"]` == `["shift","ctrl"]`).
- Different modifier sets are tracked independently; multiple toggle entries may coexist.

#### Count `{ count: N }`
- Activates M with a countdown of N.
- Each **key-dispatching trigger** (see §3) decrements the countdown by 1.
- When the countdown reaches 0 the entry is removed.
- Multiple count entries for the same M set may coexist (e.g. firing the same action
  twice creates two independent countdowns that are both tracked).

#### Timeout `{ timeout_ms: T }`
- Activates M with a deadline of `now + T ms`.
- When `check_timeout(now)` observes `now ≥ deadline`, the entry is silently removed.
- No action is fired on expiry (contrast: `PushLayer` fires `on_exit`; there is no
  equivalent here).
- `ComboEngine::next_deadline()` **must** include all active HoldModifier deadlines.

---

## 3. Engine State

`ComboEngine` gains a new field:

```rust
held_modifiers: Vec<HeldModifierEntry>
```

where:

```rust
struct HeldModifierEntry {
    modifiers: Vec<Modifier>,   // sorted for comparison
    mode: ActiveHoldMode,
}

enum ActiveHoldMode {
    Toggle,
    Count { remaining: u32 },
    Timeout { deadline: Instant },
}
```

### Orthogonality to the layer stack

`held_modifiers` is **not** cleared on `push_layer`, `pop_layer`, or `switch_layer`.
Modifier state is entirely independent of which layer is active. Rationale: the user
holds a modifier, navigates to a different layer, and expects the modifier to still
apply to the next keystroke on that new layer.

---

## 4. Application to Actions

When the engine resolves a trigger and dispatches an action, it first computes the
**effective held modifier set**: the union of all `modifiers` across all active
`HeldModifierEntry` items.

| Action type      | Held modifiers applied?             | Decrements count? |
|------------------|-------------------------------------|-------------------|
| `Key`            | Yes — unioned with action's own `modifiers` | Yes               |
| `KeyChord`       | Yes — all held modifiers added to the keys list | Yes               |
| `TypeString`     | **No** — emitted via OS text API, not keystroke simulation | Yes |
| `Macro`          | Applied to each `Key`/`KeyChord` step within the macro | Yes (1 decrement for the whole macro, not per step) |
| `HoldModifier`   | N/A — updates state only                    | No                |
| `PushLayer` / `PopLayer` / `SwitchLayer` | N/A              | No                |
| `SetVariable` / `ToggleVariable` | N/A                      | No                |
| `Block`          | N/A                                         | No                |
| `Alias`          | Resolved first; result follows its own row  | —                 |

> **Note on `TypeString`:** The modifier is not applied because `enigo::text()` emits
> the literal string via the OS clipboard/input path, bypassing physical key simulation.
> Users who need a shifted character should use `Key { key: "a", modifiers: [] }` with
> a held `Shift` modifier rather than `TypeString { text: "a" }`.

> **Note on `Macro` count decrement:** A macro counts as **one trigger dispatch** for
> count purposes, regardless of how many key steps it contains. This keeps the
> user-visible mental model simple: "one tap consumes one count".

---

## 5. Validation Rules

These rules are enforced by `Profile::validate()`:

1. `hold_modifier.modifiers` must be non-empty.
   Error: `"hold_modifier action must specify at least one modifier"`
2. `hold_modifier.modifiers` must contain no duplicate entries.
   Error: `"hold_modifier action contains duplicate modifier '{x}'"`
3. `hold_modifier.count` must be ≥ 1.
   Error: `"hold_modifier count must be at least 1"`
4. `hold_modifier.timeout_ms` must be ≥ 1.
   Error: `"hold_modifier timeout_ms must be at least 1"`
5. `HoldModifier` must not appear as an action inside a `Macro` step.
   Error: `"hold_modifier may not be used inside a macro step"`
   Rationale: macros are atomic key sequences; injecting state side-effects mid-macro
   is confusing and hard to reason about.

---

## 6. Interaction with Existing Features

### 6.1 Combined modifiers

`Action::Key` already has its own `modifiers: Vec<Modifier>` field. When held modifiers
are present, the final modifier set sent to `enigo` is:

```
effective = action.modifiers ∪ held_modifier_set
```

Duplicates are deduplicated before calling enigo.

### 6.2 ToggleVariable branching

Both `on_true` and `on_false` branches of `ToggleVariable` are subject to held
modifiers in the normal way (they resolve to a concrete action first, then §4 applies).

### 6.3 Alias resolution

`Alias` actions are resolved one level before dispatch; the resolved action then
follows the rules in §4.

### 6.4 Sequence triggers

Sequence triggers fire one action per step. Each step is a separate trigger dispatch;
count-mode entries decrement once per step.

---

## 7. Files to Create / Modify

### New files
| File | Purpose |
|------|---------|
| `crates/mapping-core/src/types/hold_modifier_mode.rs` | `HoldModifierMode` enum (mirrors `push_layer_mode.rs`) |

### Modified files
| File | Change |
|------|--------|
| `crates/mapping-core/src/types/action.rs` | Add `HoldModifier` variant; update nesting rules doc comment |
| `crates/mapping-core/src/types/mod.rs` | Re-export `HoldModifierMode` |
| `crates/mapping-core/src/engine/combo_engine.rs` | Add `held_modifiers: Vec<HeldModifierEntry>`; apply in dispatch; decrement counts; handle toggle activate/deactivate; update `next_deadline()` |
| `crates/mapping-core/src/types/profile.rs` | Add validation rules from §5 |
| `crates/mapping-core/src/error.rs` | Add error variants for hold_modifier validation |
| `src/lib/types.ts` | Add `HoldModifier` to the `Action` discriminated union; add `HoldModifierMode` type |

---

## 8. Testing Requirements

### Unit tests — `types/hold_modifier_mode.rs`
- Serialise each of the three modes; verify `"mode"` tag.
- Round-trip all three modes.
- Deserialise from raw JSON strings (spec examples).

### Unit tests — `types/action.rs`
- Serialise `HoldModifier` with each of the three modes; verify `"type":"hold_modifier"`.
- Round-trip all three modes.
- Deserialise from raw JSON strings.

### Unit tests — `engine/combo_engine.rs` (in `tests/combo_engine.rs`)
- `hold_modifier_toggle_activates_on_first_dispatch`
- `hold_modifier_toggle_deactivates_on_second_dispatch_same_set`
- `hold_modifier_toggle_two_different_sets_independent`
- `hold_modifier_count_one_applies_modifier_to_first_key_only`
- `hold_modifier_count_two_applies_modifier_to_two_keys`
- `hold_modifier_count_type_string_decrements_count_without_applying_modifier`
- `hold_modifier_timeout_applies_within_window`
- `hold_modifier_timeout_does_not_apply_after_expiry`
- `hold_modifier_combines_with_action_own_modifiers`
- `hold_modifier_survives_push_layer`
- `hold_modifier_survives_pop_layer`
- `hold_modifier_macro_counts_as_single_decrement`

### Validation tests — `types/profile.rs`
- `hold_modifier_empty_modifiers_fails_validation`
- `hold_modifier_duplicate_modifiers_fails_validation`
- `hold_modifier_count_zero_fails_validation`
- `hold_modifier_timeout_ms_zero_fails_validation`
- `hold_modifier_inside_macro_fails_validation`
- `hold_modifier_valid_toggle_passes_validation`

---

## 9. Out of Scope for This Feature

- **UI indicator for active modifier state**: Surfacing held modifier state is a
  separate UI feature and not required for this implementation.
- **Android / BLE layer changes**: `hold_modifier` is a pure engine concern.
- **Capslock-style OS modifier**: This feature operates entirely within mapxr's engine.
