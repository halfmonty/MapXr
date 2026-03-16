use serde::{Deserialize, Serialize};

use crate::types::{HoldModifierMode, KeyDef, Modifier, PushLayerMode, VariableValue};

/// An action fired when a trigger matches.
///
/// Serialised as an internally-tagged JSON object with a `"type"` field:
///
/// ```json
/// { "type": "key",             "key": "a",        "modifiers": ["ctrl"] }
/// { "type": "key_chord",       "keys": ["ctrl", "alt", "delete"]        }
/// { "type": "type_string",     "text": "hello"                          }
/// { "type": "macro",           "steps": [...]                           }
/// { "type": "push_layer",      "layer": "nav", "mode": "permanent"      }
/// { "type": "pop_layer"                                                  }
/// { "type": "switch_layer",    "layer": "base"                          }
/// { "type": "toggle_variable", "variable": "muted", ...                 }
/// { "type": "set_variable",    "variable": "muted", "value": false      }
/// { "type": "block"                                                      }
/// { "type": "alias",           "name": "save"                           }
/// { "type": "hold_modifier",   "modifiers": ["shift"], "mode": "toggle" }
/// ```
///
/// # Nesting rules
///
/// - `Macro` steps may not themselves be `Macro` actions. Profile validation
///   (task 1.22) enforces this; the type does not.
/// - `Macro` steps may not be `HoldModifier` actions. Profile validation enforces this.
/// - `ToggleVariable` may contain any action including another `ToggleVariable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Press and release a single key, optionally with modifier keys held.
    Key {
        /// The key to press.
        key: KeyDef,
        /// Modifier keys held during the press. Omitted from JSON when empty.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        modifiers: Vec<Modifier>,
    },

    /// Press all listed keys simultaneously.
    ///
    /// Use for combinations not expressible as a single key + modifiers,
    /// e.g. `ctrl+alt+delete`. Keys may include modifier names (`"ctrl"`,
    /// `"shift"`, `"alt"`, `"meta"`) alongside regular key names.
    KeyChord {
        /// Keys to press simultaneously.
        keys: Vec<String>,
    },

    /// Emit a literal string character-by-character via the platform input API.
    TypeString {
        /// The text to type.
        text: String,
    },

    /// Execute an ordered list of actions with optional delays between steps.
    ///
    /// Macro steps may not themselves be `Macro` actions (no nesting).
    /// Profile validation enforces this constraint.
    Macro {
        /// Ordered sequence of actions and delays.
        steps: Vec<MacroStep>,
    },

    /// Push a layer onto the stack.
    PushLayer {
        /// `layer_id` of the layer to push.
        layer: String,
        /// How long the pushed layer stays on the stack.
        #[serde(flatten)]
        mode: PushLayerMode,
    },

    /// Return to the previous layer. No-op if already at the base layer.
    PopLayer,

    /// Replace the entire stack with a single layer.
    SwitchLayer {
        /// `layer_id` of the layer to switch to.
        layer: String,
    },

    /// Read a boolean variable and fire one of two child actions, then flip the variable.
    ToggleVariable {
        /// Name of the variable to read and flip.
        variable: String,
        /// Action to fire when the variable is currently `true`.
        on_true: Box<Action>,
        /// Action to fire when the variable is currently `false`.
        on_false: Box<Action>,
    },

    /// Explicitly set a variable to a value without toggling.
    SetVariable {
        /// Name of the variable to set.
        variable: String,
        /// The value to assign.
        value: VariableValue,
    },

    /// Consume the tap code and fire nothing. Stops passthrough walk at this layer.
    Block,

    /// Reference a named action defined in the profile's `aliases` map.
    Alias {
        /// Name of the alias to resolve.
        name: String,
    },

    /// Activate a sticky modifier key that is applied to subsequent key actions.
    ///
    /// The modifier stays active according to `mode` (toggle, count, or timeout).
    /// When active, its keys are unioned with any `modifiers` already on a `Key`
    /// action. `HoldModifier` must not appear as a step inside a `Macro`.
    HoldModifier {
        /// Modifier keys to activate. Must be non-empty and contain no duplicates.
        modifiers: Vec<Modifier>,
        /// How long the modifier stays active.
        #[serde(flatten)]
        mode: HoldModifierMode,
    },
}

/// A single step inside a [`Action::Macro`].
///
/// Serialises as a JSON object with `"action"` and `"delay_ms"` fields:
///
/// ```json
/// { "action": { "type": "key", "key": "escape" }, "delay_ms": 0 }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroStep {
    /// The action to execute for this step.
    pub action: Action,
    /// Milliseconds to wait after this action fires before the next step begins.
    pub delay_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::VariableValue;

    fn round_trip(action: &Action) -> Action {
        let json = serde_json::to_string(action).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    // ── Action::Key ───────────────────────────────────────────────────────────

    #[test]
    fn action_key_serialises_with_correct_type_tag() {
        let a = Action::Key {
            key: KeyDef::new_unchecked("a"),
            modifiers: vec![],
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"key""#), "got: {json}");
    }

    #[test]
    fn action_key_omits_modifiers_when_empty() {
        let a = Action::Key {
            key: KeyDef::new_unchecked("a"),
            modifiers: vec![],
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(!json.contains("modifiers"), "got: {json}");
    }

    #[test]
    fn action_key_includes_modifiers_when_present() {
        let a = Action::Key {
            key: KeyDef::new_unchecked("s"),
            modifiers: vec![Modifier::Ctrl],
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(
            json.contains(r#""modifiers"["#) || json.contains(r#""modifiers":["#),
            "got: {json}"
        );
    }

    #[test]
    fn action_key_round_trips() {
        let a = Action::Key {
            key: KeyDef::new_unchecked("s"),
            modifiers: vec![Modifier::Ctrl, Modifier::Shift],
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_key_deserialises_from_profile_json() {
        let json = r#"{"type":"key","key":"a","modifiers":["ctrl","shift"]}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::Key {
                key: KeyDef::new_unchecked("a"),
                modifiers: vec![Modifier::Ctrl, Modifier::Shift],
            }
        );
    }

    #[test]
    fn action_key_deserialises_without_modifiers_field() {
        let json = r#"{"type":"key","key":"space"}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::Key {
                key: KeyDef::new_unchecked("space"),
                modifiers: vec![]
            }
        );
    }

    // ── Action::KeyChord ──────────────────────────────────────────────────────

    #[test]
    fn action_key_chord_serialises_with_correct_type_tag() {
        let a = Action::KeyChord {
            keys: vec!["ctrl".into(), "alt".into(), "delete".into()],
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"key_chord""#), "got: {json}");
    }

    #[test]
    fn action_key_chord_round_trips() {
        let a = Action::KeyChord {
            keys: vec!["ctrl".into(), "alt".into(), "delete".into()],
        };
        assert_eq!(round_trip(&a), a);
    }

    // ── Action::TypeString ────────────────────────────────────────────────────

    #[test]
    fn action_type_string_serialises_with_correct_type_tag() {
        let a = Action::TypeString {
            text: "hello".into(),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"type_string""#), "got: {json}");
    }

    #[test]
    fn action_type_string_round_trips() {
        let a = Action::TypeString {
            text: "git commit -m \"\"".into(),
        };
        assert_eq!(round_trip(&a), a);
    }

    // ── Action::Macro ─────────────────────────────────────────────────────────

    #[test]
    fn action_macro_serialises_with_correct_type_tag() {
        let a = Action::Macro {
            steps: vec![MacroStep {
                action: Action::Key {
                    key: KeyDef::new_unchecked("escape"),
                    modifiers: vec![],
                },
                delay_ms: 0,
            }],
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"macro""#), "got: {json}");
    }

    #[test]
    fn action_macro_round_trips() {
        let a = Action::Macro {
            steps: vec![
                MacroStep {
                    action: Action::Key {
                        key: KeyDef::new_unchecked("escape"),
                        modifiers: vec![],
                    },
                    delay_ms: 0,
                },
                MacroStep {
                    action: Action::TypeString { text: ":wq".into() },
                    delay_ms: 50,
                },
            ],
        };
        assert_eq!(round_trip(&a), a);
    }

    // ── Action::PushLayer ─────────────────────────────────────────────────────

    #[test]
    fn action_push_layer_permanent_serialises_correctly() {
        let a = Action::PushLayer {
            layer: "nav".into(),
            mode: PushLayerMode::Permanent,
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"push_layer""#), "got: {json}");
        assert!(json.contains(r#""mode":"permanent""#), "got: {json}");
    }

    #[test]
    fn action_push_layer_count_serialises_correctly() {
        let a = Action::PushLayer {
            layer: "nav".into(),
            mode: PushLayerMode::Count { count: 3 },
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""mode":"count""#), "got: {json}");
        assert!(json.contains(r#""count":3"#), "got: {json}");
    }

    #[test]
    fn action_push_layer_timeout_serialises_correctly() {
        let a = Action::PushLayer {
            layer: "nav".into(),
            mode: PushLayerMode::Timeout { timeout_ms: 2000 },
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""mode":"timeout""#), "got: {json}");
        assert!(json.contains(r#""timeout_ms":2000"#), "got: {json}");
    }

    #[test]
    fn action_push_layer_permanent_round_trips() {
        let a = Action::PushLayer {
            layer: "nav".into(),
            mode: PushLayerMode::Permanent,
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_push_layer_count_round_trips() {
        let a = Action::PushLayer {
            layer: "nav".into(),
            mode: PushLayerMode::Count { count: 5 },
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_push_layer_timeout_round_trips() {
        let a = Action::PushLayer {
            layer: "nav".into(),
            mode: PushLayerMode::Timeout { timeout_ms: 2000 },
        };
        assert_eq!(round_trip(&a), a);
    }

    // ── Action::PopLayer ──────────────────────────────────────────────────────

    #[test]
    fn action_pop_layer_serialises_with_correct_type_tag() {
        let json = serde_json::to_string(&Action::PopLayer).unwrap();
        assert!(json.contains(r#""type":"pop_layer""#), "got: {json}");
    }

    #[test]
    fn action_pop_layer_round_trips() {
        assert_eq!(round_trip(&Action::PopLayer), Action::PopLayer);
    }

    // ── Action::SwitchLayer ───────────────────────────────────────────────────

    #[test]
    fn action_switch_layer_serialises_with_correct_type_tag() {
        let a = Action::SwitchLayer {
            layer: "base".into(),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"switch_layer""#), "got: {json}");
    }

    #[test]
    fn action_switch_layer_round_trips() {
        let a = Action::SwitchLayer {
            layer: "base".into(),
        };
        assert_eq!(round_trip(&a), a);
    }

    // ── Action::ToggleVariable ────────────────────────────────────────────────

    #[test]
    fn action_toggle_variable_serialises_with_correct_type_tag() {
        let a = Action::ToggleVariable {
            variable: "muted".into(),
            on_true: Box::new(Action::Key {
                key: KeyDef::new_unchecked("f13"),
                modifiers: vec![],
            }),
            on_false: Box::new(Action::Key {
                key: KeyDef::new_unchecked("f14"),
                modifiers: vec![],
            }),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"toggle_variable""#), "got: {json}");
    }

    #[test]
    fn action_toggle_variable_round_trips() {
        let a = Action::ToggleVariable {
            variable: "muted".into(),
            on_true: Box::new(Action::Key {
                key: KeyDef::new_unchecked("f13"),
                modifiers: vec![],
            }),
            on_false: Box::new(Action::Block),
        };
        assert_eq!(round_trip(&a), a);
    }

    // ── Action::SetVariable ───────────────────────────────────────────────────

    #[test]
    fn action_set_variable_bool_round_trips() {
        let a = Action::SetVariable {
            variable: "muted".into(),
            value: VariableValue::Bool(false),
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_set_variable_int_round_trips() {
        let a = Action::SetVariable {
            variable: "count".into(),
            value: VariableValue::Int(42),
        };
        assert_eq!(round_trip(&a), a);
    }

    // ── Action::Block ─────────────────────────────────────────────────────────

    #[test]
    fn action_block_serialises_with_correct_type_tag() {
        let json = serde_json::to_string(&Action::Block).unwrap();
        assert!(json.contains(r#""type":"block""#), "got: {json}");
    }

    #[test]
    fn action_block_round_trips() {
        assert_eq!(round_trip(&Action::Block), Action::Block);
    }

    // ── Action::Alias ─────────────────────────────────────────────────────────

    #[test]
    fn action_alias_serialises_with_correct_type_tag() {
        let a = Action::Alias {
            name: "save".into(),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"alias""#), "got: {json}");
    }

    #[test]
    fn action_alias_round_trips() {
        let a = Action::Alias {
            name: "save".into(),
        };
        assert_eq!(round_trip(&a), a);
    }

    // ── Action::HoldModifier ──────────────────────────────────────────────────

    #[test]
    fn action_hold_modifier_toggle_serialises_with_correct_type_tag() {
        let a = Action::HoldModifier {
            modifiers: vec![Modifier::Shift],
            mode: crate::types::HoldModifierMode::Toggle,
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"hold_modifier""#), "got: {json}");
        assert!(json.contains(r#""mode":"toggle""#), "got: {json}");
    }

    #[test]
    fn action_hold_modifier_count_serialises_correctly() {
        let a = Action::HoldModifier {
            modifiers: vec![Modifier::Ctrl],
            mode: crate::types::HoldModifierMode::Count { count: 1 },
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"hold_modifier""#), "got: {json}");
        assert!(json.contains(r#""mode":"count""#), "got: {json}");
        assert!(json.contains(r#""count":1"#), "got: {json}");
    }

    #[test]
    fn action_hold_modifier_timeout_serialises_correctly() {
        let a = Action::HoldModifier {
            modifiers: vec![Modifier::Alt],
            mode: crate::types::HoldModifierMode::Timeout { timeout_ms: 2000 },
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"hold_modifier""#), "got: {json}");
        assert!(json.contains(r#""mode":"timeout""#), "got: {json}");
        assert!(json.contains(r#""timeout_ms":2000"#), "got: {json}");
    }

    #[test]
    fn action_hold_modifier_toggle_round_trips() {
        let a = Action::HoldModifier {
            modifiers: vec![Modifier::Shift],
            mode: crate::types::HoldModifierMode::Toggle,
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_hold_modifier_count_round_trips() {
        let a = Action::HoldModifier {
            modifiers: vec![Modifier::Ctrl, Modifier::Shift],
            mode: crate::types::HoldModifierMode::Count { count: 2 },
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_hold_modifier_timeout_round_trips() {
        let a = Action::HoldModifier {
            modifiers: vec![Modifier::Alt],
            mode: crate::types::HoldModifierMode::Timeout { timeout_ms: 2000 },
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_hold_modifier_toggle_deserialises_from_spec_json() {
        let json = r#"{"type":"hold_modifier","modifiers":["shift"],"mode":"toggle"}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::HoldModifier {
                modifiers: vec![Modifier::Shift],
                mode: crate::types::HoldModifierMode::Toggle,
            }
        );
    }

    #[test]
    fn action_hold_modifier_count_deserialises_from_spec_json() {
        let json = r#"{"type":"hold_modifier","modifiers":["ctrl"],"mode":"count","count":1}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::HoldModifier {
                modifiers: vec![Modifier::Ctrl],
                mode: crate::types::HoldModifierMode::Count { count: 1 },
            }
        );
    }

    #[test]
    fn action_hold_modifier_multi_modifier_set_round_trips() {
        let a = Action::HoldModifier {
            modifiers: vec![Modifier::Ctrl, Modifier::Shift],
            mode: crate::types::HoldModifierMode::Toggle,
        };
        assert_eq!(round_trip(&a), a);
    }

    // ── Deserialise from spec examples ────────────────────────────────────────

    #[test]
    fn action_push_layer_deserialises_from_spec_json_permanent() {
        let json = r#"{"type":"push_layer","layer":"nav","mode":"permanent"}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::PushLayer {
                layer: "nav".into(),
                mode: PushLayerMode::Permanent
            }
        );
    }

    #[test]
    fn action_push_layer_deserialises_from_spec_json_count() {
        let json = r#"{"type":"push_layer","layer":"nav","mode":"count","count":3}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::PushLayer {
                layer: "nav".into(),
                mode: PushLayerMode::Count { count: 3 }
            }
        );
    }

    #[test]
    fn action_push_layer_deserialises_from_spec_json_timeout() {
        let json = r#"{"type":"push_layer","layer":"nav","mode":"timeout","timeout_ms":2000}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::PushLayer {
                layer: "nav".into(),
                mode: PushLayerMode::Timeout { timeout_ms: 2000 }
            }
        );
    }
}
