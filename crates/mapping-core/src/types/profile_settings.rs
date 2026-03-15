use serde::{Deserialize, Serialize};

use crate::types::{Action, OverloadStrategy};

/// Per-profile timing and behaviour overrides.
///
/// All fields are optional. Omitted fields fall back to engine defaults.
/// Serialises as a flat JSON object nested under the `"settings"` key:
///
/// ```json
/// {
///   "settings": {
///     "combo_window_ms": 150,
///     "sequence_window_ms": 500,
///     "double_tap_window_ms": 250,
///     "triple_tap_window_ms": 400,
///     "overload_strategy": "patient",
///     "eager_undo_sequence": [{ "type": "key", "key": "backspace" }]
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProfileSettings {
    /// How long (ms) to wait for a cross-device chord before resolving pending
    /// events as individual taps. Applies to dual profiles only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combo_window_ms: Option<u64>,

    /// Maximum time (ms) between steps in a `sequence` trigger.
    /// Per-trigger `window_ms` overrides this for individual sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence_window_ms: Option<u64>,

    /// Maximum time (ms) between the first and second tap of a `double_tap`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_tap_window_ms: Option<u64>,

    /// Maximum time (ms) from first to third tap of a `triple_tap`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triple_tap_window_ms: Option<u64>,

    /// How to resolve overloaded codes (same code bound to both `tap` and
    /// `double_tap` / `triple_tap`). Required when any overloaded code exists
    /// in the profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overload_strategy: Option<OverloadStrategy>,

    /// Actions used to undo an eagerly-fired single-tap before firing the
    /// double-tap action. Only relevant when `overload_strategy` is `eager`.
    /// Defaults to a single backspace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eager_undo_sequence: Option<Vec<Action>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{KeyDef, OverloadStrategy};

    fn round_trip(s: &ProfileSettings) -> ProfileSettings {
        let json = serde_json::to_string(s).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn profile_settings_all_none_serialises_as_empty_object() {
        let s = ProfileSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn profile_settings_combo_window_round_trips() {
        let s = ProfileSettings {
            combo_window_ms: Some(150),
            ..Default::default()
        };
        assert_eq!(round_trip(&s), s);
    }

    #[test]
    fn profile_settings_sequence_window_round_trips() {
        let s = ProfileSettings {
            sequence_window_ms: Some(500),
            ..Default::default()
        };
        assert_eq!(round_trip(&s), s);
    }

    #[test]
    fn profile_settings_double_tap_window_round_trips() {
        let s = ProfileSettings {
            double_tap_window_ms: Some(250),
            ..Default::default()
        };
        assert_eq!(round_trip(&s), s);
    }

    #[test]
    fn profile_settings_triple_tap_window_round_trips() {
        let s = ProfileSettings {
            triple_tap_window_ms: Some(400),
            ..Default::default()
        };
        assert_eq!(round_trip(&s), s);
    }

    #[test]
    fn profile_settings_overload_strategy_round_trips() {
        let s = ProfileSettings {
            overload_strategy: Some(OverloadStrategy::Patient),
            ..Default::default()
        };
        assert_eq!(round_trip(&s), s);
    }

    #[test]
    fn profile_settings_eager_undo_sequence_round_trips() {
        let s = ProfileSettings {
            overload_strategy: Some(OverloadStrategy::Eager),
            eager_undo_sequence: Some(vec![Action::Key {
                key: KeyDef::new_unchecked("backspace"),
                modifiers: vec![],
            }]),
            ..Default::default()
        };
        assert_eq!(round_trip(&s), s);
    }

    #[test]
    fn profile_settings_deserialises_from_spec_json() {
        let json = r#"{
            "combo_window_ms": 150,
            "sequence_window_ms": 500,
            "double_tap_window_ms": 250,
            "overload_strategy": "eager",
            "eager_undo_sequence": [{ "type": "key", "key": "backspace" }]
        }"#;
        let s: ProfileSettings = serde_json::from_str(json).unwrap();
        assert_eq!(s.combo_window_ms, Some(150));
        assert_eq!(s.sequence_window_ms, Some(500));
        assert_eq!(s.double_tap_window_ms, Some(250));
        assert_eq!(s.overload_strategy, Some(OverloadStrategy::Eager));
        assert!(s.eager_undo_sequence.is_some());
    }
}
