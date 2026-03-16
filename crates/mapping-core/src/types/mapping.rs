use serde::{Deserialize, Serialize};

use crate::types::{Action, Trigger};

fn default_enabled() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

/// A runtime condition that guards a mapping.
///
/// The mapping is only considered when the named variable on the current top
/// layer equals `value`. If the variable is absent the condition is not met.
///
/// ```json
/// { "variable": "caps_lock", "value": true }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MappingCondition {
    /// Name of the variable to test.
    pub variable: String,
    /// The boolean value the variable must equal.
    pub value: bool,
}

/// A single input binding: a trigger paired with an action.
///
/// Serialises as a JSON object inside the profile `"mappings"` array:
///
/// ```json
/// {
///   "label": "Thumb → Space",
///   "trigger": { "type": "tap", "code": "xoooo" },
///   "action":  { "type": "key", "key": "space" }
/// }
/// ```
///
/// The `enabled` field defaults to `true` and is omitted from JSON when true,
/// so existing profile files without the field deserialise correctly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    /// Human-readable label shown in the UI and debug output.
    pub label: String,

    /// The input event pattern that triggers this mapping.
    pub trigger: Trigger,

    /// The action to fire when the trigger matches.
    pub action: Action,

    /// Whether this mapping is active. Defaults to `true`. Set to `false` to
    /// disable a mapping without deleting it.
    #[serde(default = "default_enabled", skip_serializing_if = "is_true")]
    pub enabled: bool,

    /// Optional variable condition. When set, this mapping is only considered
    /// when the named variable on the current top layer equals `value`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<MappingCondition>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{KeyDef, TriggerPattern};

    fn sample_mapping() -> Mapping {
        Mapping {
            label: "Thumb → Space".into(),
            trigger: Trigger::Tap {
                code: TriggerPattern::Single(crate::types::TapCode::from_u8(1).unwrap()),
            },
            action: Action::Key {
                key: KeyDef::new_unchecked("space"),
                modifiers: vec![],
            },
            enabled: true,
            condition: None,
        }
    }

    fn round_trip(m: &Mapping) -> Mapping {
        let json = serde_json::to_string(m).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn mapping_round_trips() {
        let m = sample_mapping();
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn mapping_enabled_true_omitted_from_json() {
        let m = sample_mapping();
        let json = serde_json::to_string(&m).unwrap();
        assert!(!json.contains("enabled"), "got: {json}");
    }

    #[test]
    fn mapping_enabled_false_present_in_json() {
        let m = Mapping {
            enabled: false,
            ..sample_mapping()
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"enabled\":false"), "got: {json}");
    }

    #[test]
    fn mapping_deserialises_without_enabled_field_defaults_to_true() {
        let json = r#"{
            "label": "test",
            "trigger": { "type": "tap", "code": "xoooo" },
            "action": { "type": "key", "key": "space" }
        }"#;
        let m: Mapping = serde_json::from_str(json).unwrap();
        assert!(m.enabled);
    }

    #[test]
    fn mapping_deserialises_enabled_false_from_json() {
        let json = r#"{
            "label": "test",
            "trigger": { "type": "tap", "code": "xoooo" },
            "action": { "type": "key", "key": "space" },
            "enabled": false
        }"#;
        let m: Mapping = serde_json::from_str(json).unwrap();
        assert!(!m.enabled);
    }

    #[test]
    fn mapping_deserialises_from_spec_json() {
        let json = r#"{
            "label": "Unpause",
            "trigger": { "type": "tap", "code": "xxxxx xxxxx" },
            "action": { "type": "pop_layer" }
        }"#;
        let m: Mapping = serde_json::from_str(json).unwrap();
        assert_eq!(m.label, "Unpause");
        assert_eq!(m.action, Action::PopLayer);
        assert!(m.enabled);
    }

    #[test]
    fn mapping_condition_none_omitted_from_json() {
        let m = sample_mapping();
        let json = serde_json::to_string(&m).unwrap();
        assert!(!json.contains("condition"), "got: {json}");
    }

    #[test]
    fn mapping_condition_serialises_when_set() {
        let m = Mapping {
            condition: Some(MappingCondition {
                variable: "caps_lock".into(),
                value: true,
            }),
            ..sample_mapping()
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"condition\""), "got: {json}");
        assert!(json.contains("\"caps_lock\""), "got: {json}");
    }

    #[test]
    fn mapping_condition_round_trips() {
        let m = Mapping {
            condition: Some(MappingCondition {
                variable: "caps_lock".into(),
                value: false,
            }),
            ..sample_mapping()
        };
        let json = serde_json::to_string(&m).unwrap();
        let m2: Mapping = serde_json::from_str(&json).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn mapping_deserialises_without_condition_defaults_to_none() {
        let json = r#"{
            "label": "test",
            "trigger": { "type": "tap", "code": "xoooo" },
            "action": { "type": "key", "key": "space" }
        }"#;
        let m: Mapping = serde_json::from_str(json).unwrap();
        assert!(m.condition.is_none());
    }
}
