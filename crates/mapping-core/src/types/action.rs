use serde::{Deserialize, Serialize};

use crate::types::{
    HoldModifierMode, KeyDef, Modifier, MouseButton, PushLayerMode, ScrollDirection, VariableValue,
};

// ── VibrationPattern ──────────────────────────────────────────────────────────

/// A haptic vibration pattern: alternating on/off durations in milliseconds.
///
/// Elements alternate **on, off, on, off, …** starting with on at index 0.
/// Each duration may be in the range 10–2550 ms with 10 ms resolution; values
/// outside this range are clamped silently during BLE encoding, not rejected.
///
/// A maximum of 18 elements (9 on/off pairs) can be sent per BLE write;
/// longer sequences are truncated at the send site.
///
/// An empty pattern is a no-op.
///
/// See `docs/spec/haptics-spec.md` §VibrationPattern for full encoding rules.
///
/// # JSON example
/// ```json
/// [200, 100, 200, 100, 200]
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VibrationPattern(pub Vec<u16>);

impl VibrationPattern {
    /// Encode the pattern into the BLE payload written to the haptic characteristic.
    ///
    /// Payload layout: always exactly 20 bytes — `[0x00, 0x02, d0, d1, …, d17]` where
    /// each duration byte is `clamp(duration_ms / 10, 0, 255)` and unused slots are
    /// zero-filled.  The fixed 20-byte size is required by the device firmware; sending
    /// a shorter payload leaves the trailing slots as uninitialised device RAM, which
    /// the firmware reads as additional phantom durations.  The maximum 18-element limit
    /// is enforced by truncation before encoding.
    ///
    /// Returns an empty `Vec` if the pattern has no elements (no write is issued).
    ///
    /// Source: `docs/reference/vibration.txt` (Tap Python SDK) and C# SDK implementation.
    pub fn encode(&self) -> Vec<u8> {
        if self.0.is_empty() {
            return Vec::new();
        }
        // The device firmware expects exactly 20 bytes: 2-byte header + 18 duration slots.
        // Unused slots must be explicitly zeroed; the device does not zero-pad internally
        // and will read uninitialised buffer contents as additional on/off durations if the
        // payload is shorter than 20 bytes.  This matches the C# SDK implementation.
        let mut payload = vec![0u8; 20];
        payload[0] = 0x00; // reserved prefix
        payload[1] = 0x02; // vibration sub-command
        for (i, &d) in self.0.iter().take(18).enumerate() {
            payload[2 + i] = (d / 10).min(255) as u8;
        }
        payload
    }
}

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
/// { "type": "conditional",     "variable": "caps", "on_true": {...}, "on_false": {...} }
/// { "type": "block"                                                      }
/// { "type": "alias",           "name": "save"                           }
/// { "type": "hold_modifier",   "modifiers": ["shift"], "mode": "toggle" }
/// { "type": "mouse_click",        "button": "left"                      }
/// { "type": "mouse_double_click", "button": "right"                     }
/// { "type": "mouse_scroll",       "direction": "down"                   }
/// { "type": "vibrate",            "pattern": [200, 100, 200]            }
/// ```
///
/// # Nesting rules
///
/// - `Macro` steps may not themselves be `Macro` actions. Profile validation
///   (task 1.22) enforces this; the type does not.
/// - `Macro` steps may not be `HoldModifier` actions. Profile validation enforces this.
/// - `ToggleVariable` and `Conditional` may contain any action including further nesting.
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

    /// Read a boolean variable and dispatch one of two child actions without
    /// modifying the variable.
    ///
    /// If the variable does not exist, `on_false` is dispatched.
    Conditional {
        /// Name of the variable to read from the current top layer.
        variable: String,
        /// Action dispatched when the variable is `true`.
        on_true: Box<Action>,
        /// Action dispatched when the variable is `false` or absent.
        on_false: Box<Action>,
    },

    /// Consume the tap code and fire nothing. Stops passthrough walk at this layer.
    Block,

    /// Reference a named action defined in the profile's `aliases` map.
    Alias {
        /// Name of the alias to resolve.
        name: String,
    },

    /// Click a mouse button once.
    MouseClick {
        /// The button to click.
        button: MouseButton,
    },

    /// Double-click a mouse button.
    MouseDoubleClick {
        /// The button to click.
        button: MouseButton,
    },

    /// Scroll in a cardinal direction by one platform scroll unit.
    MouseScroll {
        /// The direction to scroll.
        direction: ScrollDirection,
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

    /// Send a vibration pattern to all connected Tap devices.
    ///
    /// Dispatched via [`tap_ble::TapDevice::vibrate`] to every currently
    /// connected device. Silently dropped if no device is connected.
    Vibrate {
        /// Alternating on/off durations in milliseconds.
        pattern: VibrationPattern,
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
    use crate::types::{MouseButton, ScrollDirection, VariableValue};

    // ── VibrationPattern::encode ──────────────────────────────────────────────

    #[test]
    fn vibration_pattern_encode_empty_returns_empty_vec() {
        assert_eq!(VibrationPattern(vec![]).encode(), Vec::<u8>::new());
    }

    fn padded(header: &[u8]) -> Vec<u8> {
        // Build the expected 20-byte payload: provided header bytes followed by zeros.
        let mut v = header.to_vec();
        v.resize(20, 0);
        v
    }

    #[test]
    fn vibration_pattern_encode_sdk_example_matches_expected_bytes() {
        // Python SDK example: [1000, 300, 200] → header [0x00, 0x02, 100, 30, 20]
        // padded to 20 bytes.  Source: docs/reference/vibration.txt
        assert_eq!(
            VibrationPattern(vec![1000, 300, 200]).encode(),
            padded(&[0x00, 0x02, 100, 30, 20])
        );
    }

    #[test]
    fn vibration_pattern_encode_single_on_pulse() {
        assert_eq!(
            VibrationPattern(vec![80]).encode(),
            padded(&[0x00, 0x02, 8])
        );
    }

    #[test]
    fn vibration_pattern_encode_clamps_above_max_duration() {
        // 2560 ms > 2550 ms max → encoded as 255
        assert_eq!(
            VibrationPattern(vec![2560]).encode(),
            padded(&[0x00, 0x02, 255])
        );
    }

    #[test]
    fn vibration_pattern_encode_resolution_floors_to_nearest_10ms() {
        // 15 ms → 15 / 10 = 1; 19 ms → 19 / 10 = 1
        assert_eq!(
            VibrationPattern(vec![15, 19]).encode(),
            padded(&[0x00, 0x02, 1, 1])
        );
    }

    #[test]
    fn vibration_pattern_encode_zero_encodes_to_zero_byte() {
        assert_eq!(
            VibrationPattern(vec![0]).encode(),
            padded(&[0x00, 0x02, 0])
        );
    }

    #[test]
    fn vibration_pattern_encode_truncates_at_18_elements() {
        let durations: Vec<u16> = (1u16..=22).map(|i| i * 10).collect(); // 22 elements
        let encoded = VibrationPattern(durations).encode();
        assert_eq!(encoded.len(), 20, "header(2) + 18 durations");
        assert_eq!(&encoded[..2], &[0x00, 0x02]);
        for i in 0..18usize {
            assert_eq!(encoded[2 + i], (i + 1) as u8, "element {i} mismatch");
        }
    }

    #[test]
    fn vibration_pattern_encode_exactly_18_elements_not_truncated() {
        let encoded = VibrationPattern(vec![100; 18]).encode();
        assert_eq!(encoded.len(), 20);
        assert!(encoded[2..].iter().all(|&b| b == 10));
    }

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

    // ── Action::MouseClick ────────────────────────────────────────────────────

    #[test]
    fn action_mouse_click_serialises_with_correct_type_tag() {
        let a = Action::MouseClick {
            button: MouseButton::Left,
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"mouse_click""#), "got: {json}");
    }

    #[test]
    fn action_mouse_click_left_round_trips() {
        let a = Action::MouseClick {
            button: MouseButton::Left,
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_mouse_click_right_round_trips() {
        let a = Action::MouseClick {
            button: MouseButton::Right,
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_mouse_click_middle_round_trips() {
        let a = Action::MouseClick {
            button: MouseButton::Middle,
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_mouse_click_deserialises_from_spec_json() {
        let json = r#"{"type":"mouse_click","button":"left"}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::MouseClick {
                button: MouseButton::Left
            }
        );
    }

    #[test]
    fn action_mouse_click_unknown_button_returns_error() {
        let json = r#"{"type":"mouse_click","button":"thumb"}"#;
        let result: Result<Action, _> = serde_json::from_str(json);
        assert!(result.is_err(), "expected error for unknown button name");
    }

    // ── Action::MouseDoubleClick ──────────────────────────────────────────────

    #[test]
    fn action_mouse_double_click_serialises_with_correct_type_tag() {
        let a = Action::MouseDoubleClick {
            button: MouseButton::Left,
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(
            json.contains(r#""type":"mouse_double_click""#),
            "got: {json}"
        );
    }

    #[test]
    fn action_mouse_double_click_right_round_trips() {
        let a = Action::MouseDoubleClick {
            button: MouseButton::Right,
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_mouse_double_click_deserialises_from_spec_json() {
        let json = r#"{"type":"mouse_double_click","button":"right"}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::MouseDoubleClick {
                button: MouseButton::Right
            }
        );
    }

    // ── Action::MouseScroll ───────────────────────────────────────────────────

    #[test]
    fn action_mouse_scroll_serialises_with_correct_type_tag() {
        let a = Action::MouseScroll {
            direction: ScrollDirection::Down,
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"mouse_scroll""#), "got: {json}");
    }

    #[test]
    fn action_mouse_scroll_up_round_trips() {
        let a = Action::MouseScroll {
            direction: ScrollDirection::Up,
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_mouse_scroll_down_round_trips() {
        let a = Action::MouseScroll {
            direction: ScrollDirection::Down,
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_mouse_scroll_left_round_trips() {
        let a = Action::MouseScroll {
            direction: ScrollDirection::Left,
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_mouse_scroll_right_round_trips() {
        let a = Action::MouseScroll {
            direction: ScrollDirection::Right,
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_mouse_scroll_deserialises_from_spec_json() {
        let json = r#"{"type":"mouse_scroll","direction":"up"}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::MouseScroll {
                direction: ScrollDirection::Up
            }
        );
    }

    #[test]
    fn action_mouse_scroll_unknown_direction_returns_error() {
        let json = r#"{"type":"mouse_scroll","direction":"diagonal"}"#;
        let result: Result<Action, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "expected error for unknown scroll direction"
        );
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

    // ── VibrationPattern serde ────────────────────────────────────────────────

    #[test]
    fn vibration_pattern_serialises_as_json_array() {
        let p = VibrationPattern(vec![200, 100, 200]);
        let json = serde_json::to_string(&p).expect("serialize");
        assert_eq!(json, "[200,100,200]");
    }

    #[test]
    fn vibration_pattern_deserialises_from_json_array() {
        let p: VibrationPattern = serde_json::from_str("[200,100,200]").expect("deserialize");
        assert_eq!(p, VibrationPattern(vec![200, 100, 200]));
    }

    #[test]
    fn vibration_pattern_empty_round_trips_as_empty_array() {
        let p = VibrationPattern(vec![]);
        let json = serde_json::to_string(&p).expect("serialize");
        assert_eq!(json, "[]");
        let back: VibrationPattern = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, p);
    }

    #[test]
    fn vibration_pattern_boundary_values_round_trip() {
        // 0 (min), 2550 (10ms-resolution max), and 65535 (u16::MAX, clamped on encode)
        let p = VibrationPattern(vec![0, 2550, 65535]);
        let json = serde_json::to_string(&p).expect("serialize");
        let back: VibrationPattern = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, p);
    }

    #[test]
    fn vibration_pattern_single_element_round_trips() {
        let p = VibrationPattern(vec![80]);
        let json = serde_json::to_string(&p).expect("serialize");
        let back: VibrationPattern = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, p);
    }

    // ── Action::Vibrate ───────────────────────────────────────────────────────

    #[test]
    fn action_vibrate_serialises_with_correct_type_tag() {
        let a = Action::Vibrate {
            pattern: VibrationPattern(vec![200, 100, 200]),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""type":"vibrate""#), "got: {json}");
    }

    #[test]
    fn action_vibrate_round_trips() {
        let a = Action::Vibrate {
            pattern: VibrationPattern(vec![200, 100, 200, 100, 200]),
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_vibrate_empty_pattern_round_trips() {
        let a = Action::Vibrate {
            pattern: VibrationPattern(vec![]),
        };
        assert_eq!(round_trip(&a), a);
    }

    #[test]
    fn action_vibrate_deserialises_from_spec_json() {
        let json = r#"{"type":"vibrate","pattern":[200,100,200]}"#;
        let a: Action = serde_json::from_str(json).unwrap();
        assert_eq!(
            a,
            Action::Vibrate {
                pattern: VibrationPattern(vec![200, 100, 200])
            }
        );
    }
}
