# Variable Conditions Spec

**Status:** Draft — awaiting user approval before implementation begins.

This document specifies two enhancements to the variable system:

1. **Conditional action** — a new `Action` variant that dispatches one of two child actions based on a boolean variable value, without modifying the variable.
2. **Variable-guarded mappings** — an optional `condition` field on `Mapping` that causes the engine to skip a mapping when the condition is not satisfied.

---

## Background

Variables are boolean values stored per-layer and initialized from `profile.variables`. The existing `toggle_variable` action reads a variable, flips it, and dispatches a child action. The existing `set_variable` action writes a value unconditionally.

Neither action allows a *separate, unrelated* tap pattern to behave differently based on variable state. These two features close that gap.

---

## 1. Conditional action

### 1.1 Purpose

Execute one of two child actions depending on the current boolean value of a named variable, without modifying the variable.

### 1.2 JSON schema

```json
{
  "type": "conditional",
  "variable": "<variable_name>",
  "on_true": <Action>,
  "on_false": <Action>
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `type` | `"conditional"` | yes | Discriminant |
| `variable` | string | yes | Name of the variable to read from the current top layer |
| `on_true` | Action | yes | Action dispatched when the variable is `true` |
| `on_false` | Action | yes | Action dispatched when the variable is `false` |

### 1.3 Engine behaviour

- The engine reads the named variable from the **top layer** via `LayerStack::get_variable`.
- If the variable is `Bool(true)`, dispatch `on_true`.
- If the variable is `Bool(false)` **or the variable does not exist**, dispatch `on_false`.
- The child action is dispatched inline (same as `ToggleVariable`'s child dispatch). It can itself be any valid `Action`, including another `Conditional`, `Macro`, `PushLayer`, etc.
- The variable is not modified.

### 1.4 Validation rules

- `variable` must be non-empty.
- `on_true` and `on_false` must each be a valid `Action` (validated recursively by the existing `Action::validate` logic).
- Circular references (`Conditional` → `Alias` → `Conditional` pointing at each other) are caught by the existing alias cycle-detection pass; no special handling needed for `Conditional` itself.

### 1.5 Rust type change

Add to the `Action` enum in `crates/mapping-core/src/types/action.rs` (or wherever `Action` is defined):

```rust
Conditional {
    variable: String,
    on_true: Box<Action>,
    on_false: Box<Action>,
},
```

Serde: internally tagged (`"type": "conditional"`), consistent with all other `Action` variants.

### 1.6 Engine dispatch (pump.rs)

In `execute_action`:

```rust
Action::Conditional { variable, on_true, on_false } => {
    let val = state.engine.lock().await.top_variables()
        .get(variable.as_str())
        .cloned();
    let child = match val {
        Some(VariableValue::Bool(true)) => on_true,
        _ => on_false,
    };
    Box::pin(execute_action(app, state, child)).await;
}
```

### 1.7 Frontend — ActionEditor

Add `"conditional"` to the action type selector. When selected, show:

- A variable selector (same dropdown used by `toggle_variable` / `set_variable`).
- Two nested `ActionEditor` instances labelled **"When true"** and **"When false"**.

The nested editors must support the full action type set (recursive nesting is valid).

### 1.8 Frontend — `action_kind_name` in pump.rs

Add `Action::Conditional { .. } => "conditional"` to the `action_kind_name` match.

---

## 2. Variable-guarded mappings

### 2.1 Purpose

Allow a mapping to be conditional on a variable value so that the same tap pattern can have different bindings depending on the current "mode" without switching layers.

### 2.2 JSON schema — Mapping

Add an optional `condition` field to `Mapping`:

```json
{
  "trigger": { ... },
  "action": { ... },
  "enabled": true,
  "condition": {
    "variable": "<variable_name>",
    "value": true
  }
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `variable` | string | yes | Name of the variable to test |
| `value` | boolean | yes | The value the variable must equal for this mapping to be active |

`condition` is optional. When absent the mapping is always active (existing behaviour).

### 2.3 Rust type change

Add to `Mapping` in `crates/mapping-core/src/types/`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub condition: Option<MappingCondition>,
```

New type:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MappingCondition {
    pub variable: String,
    pub value: bool,
}
```

### 2.4 Engine behaviour — resolution

During trigger resolution, before a mapping is considered a match, check its condition:

- If `condition` is `None` → always eligible.
- If `condition` is `Some(c)` → read `c.variable` from the current top layer. The mapping is eligible only if the variable's current value equals `c.value`. If the variable does not exist, the mapping is **not** eligible.

The condition check happens in the same pass that already filters by `enabled`. No change to trigger-matching logic or timing windows.

### 2.5 Conflict / ordering rules

Two mappings with the same trigger pattern but different conditions are both valid:

```json
[
  { "trigger": "xxxxx", "action": {"type":"key","key":"a"}, "condition": {"variable":"caps","value":false} },
  { "trigger": "xxxxx", "action": {"type":"key","key":"A"}, "condition": {"variable":"caps","value":true}  }
]
```

Resolution picks the **first eligible** mapping in declaration order, the same as the existing rule for mappings without conditions. A mapping with no condition takes precedence over a mapping with a condition if it appears first — order matters.

### 2.6 Validation rules

- `condition.variable` must be non-empty.
- If `profile.validate()` checks for undefined variables (currently via `undefinedVariables` in the frontend), the condition variable should be included in that check.
- Two mappings for the same trigger may have conflicting or redundant conditions — this is not a validation error (it is the user's responsibility to keep conditions coherent).

### 2.7 Frontend — mapping list / editor

In the mapping editor, add a **Condition** subsection below the trigger:

- A checkbox or toggle: **"Only when variable..."**
- When enabled: a variable selector and a true/false selector.
- When disabled: `condition` is omitted from the saved mapping.

The mapping list row should show a small badge (e.g. `if caps`) when a condition is set, so the user can see at a glance which mappings are guarded.

---

## Implementation order

These two features are independent and can be implemented in either order. Suggested order:

1. **Conditional action** first — it is purely additive (new action variant, no schema changes to `Mapping`), has no interaction with the resolution path, and provides immediate value.
2. **Variable-guarded mappings** second — touches the resolution path but is still contained within the engine.

---

## Out of scope for this spec

- Integer variable support in the UI (deferred; integer type remains in the engine).
- Conditions based on expressions more complex than a single variable equality check.
- Multiple conditions combined with AND/OR logic.
- Conditions on `on_enter` / `on_exit` lifecycle actions (these can use the `Conditional` action type instead).
