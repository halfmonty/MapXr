use serde::{Deserialize, Serialize};

/// Controls how long a held modifier stays active.
///
/// Used in [`Action::HoldModifier`](crate::types::Action::HoldModifier). Serialised
/// as an internally-tagged object with a `"mode"` field, flattened into the
/// parent `hold_modifier` action JSON object:
///
/// ```json
/// { "type": "hold_modifier", "modifiers": ["shift"], "mode": "toggle" }
/// { "type": "hold_modifier", "modifiers": ["ctrl"],  "mode": "count",   "count": 1        }
/// { "type": "hold_modifier", "modifiers": ["alt"],   "mode": "timeout", "timeout_ms": 2000 }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum HoldModifierMode {
    /// Toggle: first dispatch enables the modifier set; a second dispatch of the
    /// same modifier set (order-independent) disables it.
    Toggle,
    /// Active for `count` key-dispatching trigger firings, then removed.
    ///
    /// `count` must be ≥ 1 (validated by
    /// [`Profile::validate`](crate::types::Profile::validate)).
    Count {
        /// Number of key-dispatching trigger firings before the modifier is removed.
        count: u32,
    },
    /// Active until `timeout_ms` milliseconds have elapsed since activation.
    ///
    /// `timeout_ms` must be ≥ 1 (validated by
    /// [`Profile::validate`](crate::types::Profile::validate)).
    Timeout {
        /// Milliseconds before the modifier is automatically removed.
        timeout_ms: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(m: &HoldModifierMode) -> HoldModifierMode {
        let json = serde_json::to_string(m).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn hold_modifier_mode_toggle_serialises_with_correct_tag() {
        let json = serde_json::to_string(&HoldModifierMode::Toggle).unwrap();
        assert!(json.contains(r#""mode":"toggle""#), "got: {json}");
    }

    #[test]
    fn hold_modifier_mode_toggle_round_trips() {
        assert_eq!(
            round_trip(&HoldModifierMode::Toggle),
            HoldModifierMode::Toggle
        );
    }

    #[test]
    fn hold_modifier_mode_count_serialises_with_correct_tag_and_value() {
        let m = HoldModifierMode::Count { count: 1 };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains(r#""mode":"count""#), "got: {json}");
        assert!(json.contains(r#""count":1"#), "got: {json}");
    }

    #[test]
    fn hold_modifier_mode_count_round_trips() {
        let m = HoldModifierMode::Count { count: 3 };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn hold_modifier_mode_timeout_serialises_with_correct_tag_and_value() {
        let m = HoldModifierMode::Timeout { timeout_ms: 2000 };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains(r#""mode":"timeout""#), "got: {json}");
        assert!(json.contains(r#""timeout_ms":2000"#), "got: {json}");
    }

    #[test]
    fn hold_modifier_mode_timeout_round_trips() {
        let m = HoldModifierMode::Timeout { timeout_ms: 2000 };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn hold_modifier_mode_toggle_deserialises_from_spec_json() {
        let json = r#"{"mode":"toggle"}"#;
        let m: HoldModifierMode = serde_json::from_str(json).unwrap();
        assert_eq!(m, HoldModifierMode::Toggle);
    }

    #[test]
    fn hold_modifier_mode_count_deserialises_from_spec_json() {
        let json = r#"{"mode":"count","count":1}"#;
        let m: HoldModifierMode = serde_json::from_str(json).unwrap();
        assert_eq!(m, HoldModifierMode::Count { count: 1 });
    }

    #[test]
    fn hold_modifier_mode_timeout_deserialises_from_spec_json() {
        let json = r#"{"mode":"timeout","timeout_ms":2000}"#;
        let m: HoldModifierMode = serde_json::from_str(json).unwrap();
        assert_eq!(m, HoldModifierMode::Timeout { timeout_ms: 2000 });
    }
}
