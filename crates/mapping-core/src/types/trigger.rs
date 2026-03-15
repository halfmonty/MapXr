use serde::{Deserialize, Serialize};

use crate::types::TriggerPattern;

/// A trigger definition: the finger-pattern event (or sequence of events) that
/// must occur before the associated action fires.
///
/// # Serialisation
///
/// Uses an internally-tagged JSON representation with the `"type"` field:
///
/// ```json
/// { "type": "tap",        "code": "oooox" }
/// { "type": "double_tap", "code": "oooox" }
/// { "type": "triple_tap", "code": "oooox" }
/// { "type": "sequence",   "steps": ["oooox", "ooxoo"], "window_ms": 400 }
/// ```
///
/// # Left-hand profiles
///
/// `code` and `steps` fields are parsed with right-hand (thumb-first) notation
/// as the default. The profile loader re-resolves them with the correct [`Hand`]
/// context for left-hand profiles — see `Profile::load` (task 1.22).
///
/// [`Hand`]: crate::types::Hand
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    /// A single simultaneous chord.
    Tap {
        /// The finger pattern that must be tapped.
        code: TriggerPattern,
    },
    /// The same chord tapped twice within `double_tap_window_ms`.
    DoubleTap {
        /// The finger pattern that must be double-tapped.
        code: TriggerPattern,
    },
    /// The same chord tapped three times within `triple_tap_window_ms`.
    TripleTap {
        /// The finger pattern that must be triple-tapped.
        code: TriggerPattern,
    },
    /// An ordered sequence of chords, each within `sequence_window_ms` of the previous.
    Sequence {
        /// Ordered list of pattern steps.
        steps: Vec<TapStep>,
        /// Per-trigger window override. When set, overrides the profile-level
        /// `sequence_window_ms` for this trigger only.
        #[serde(skip_serializing_if = "Option::is_none")]
        window_ms: Option<u64>,
    },
}

/// A single step within a [`Trigger::Sequence`].
///
/// Serialises and deserialises as a plain finger-pattern string (not a JSON
/// object), matching the profile format: `"steps": ["oooox", "ooxoo"]`.
#[derive(Debug, Clone, PartialEq)]
pub struct TapStep {
    /// The finger pattern required for this step.
    pub code: TriggerPattern,
}

impl Serialize for TapStep {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.code.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TapStep {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let code = TriggerPattern::deserialize(deserializer)?;
        Ok(TapStep { code })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TapCode;

    fn tap_code(raw: u8) -> TriggerPattern {
        TriggerPattern::Single(TapCode::from_u8(raw).unwrap())
    }

    // ── Trigger::Tap ──────────────────────────────────────────────────────────

    #[test]
    fn trigger_tap_serialises_with_correct_type_tag() {
        let t = Trigger::Tap { code: tap_code(1) };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""type":"tap""#), "got: {json}");
    }

    #[test]
    fn trigger_tap_round_trips_via_serde() {
        let original = Trigger::Tap {
            code: tap_code(0b00001),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: Trigger = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    // ── Trigger::DoubleTap ────────────────────────────────────────────────────

    #[test]
    fn trigger_double_tap_serialises_with_correct_type_tag() {
        let t = Trigger::DoubleTap { code: tap_code(1) };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""type":"double_tap""#), "got: {json}");
    }

    #[test]
    fn trigger_double_tap_round_trips_via_serde() {
        let original = Trigger::DoubleTap {
            code: tap_code(0b00100),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: Trigger = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    // ── Trigger::TripleTap ────────────────────────────────────────────────────

    #[test]
    fn trigger_triple_tap_serialises_with_correct_type_tag() {
        let t = Trigger::TripleTap { code: tap_code(1) };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""type":"triple_tap""#), "got: {json}");
    }

    #[test]
    fn trigger_triple_tap_round_trips_via_serde() {
        let original = Trigger::TripleTap {
            code: tap_code(0b11111),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: Trigger = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    // ── Trigger::Sequence ─────────────────────────────────────────────────────

    #[test]
    fn trigger_sequence_serialises_with_correct_type_tag() {
        let t = Trigger::Sequence {
            steps: vec![TapStep { code: tap_code(1) }],
            window_ms: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""type":"sequence""#), "got: {json}");
    }

    #[test]
    fn trigger_sequence_steps_serialise_as_plain_strings() {
        let t = Trigger::Sequence {
            steps: vec![
                TapStep {
                    code: tap_code(0b00001),
                }, // thumb → "xoooo"
                TapStep {
                    code: tap_code(0b00100),
                }, // middle → "ooxoo"
            ],
            window_ms: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""xoooo""#), "got: {json}");
        assert!(json.contains(r#""ooxoo""#), "got: {json}");
    }

    #[test]
    fn trigger_sequence_window_ms_omitted_when_none() {
        let t = Trigger::Sequence {
            steps: vec![TapStep { code: tap_code(1) }],
            window_ms: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("window_ms"), "got: {json}");
    }

    #[test]
    fn trigger_sequence_window_ms_present_when_some() {
        let t = Trigger::Sequence {
            steps: vec![TapStep { code: tap_code(1) }],
            window_ms: Some(400),
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""window_ms":400"#), "got: {json}");
    }

    #[test]
    fn trigger_sequence_round_trips_via_serde() {
        let original = Trigger::Sequence {
            steps: vec![
                TapStep {
                    code: tap_code(0b00001),
                },
                TapStep {
                    code: tap_code(0b00100),
                },
            ],
            window_ms: Some(400),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: Trigger = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    // ── Deserialise from profile-style JSON ───────────────────────────────────

    #[test]
    fn trigger_tap_deserialises_from_string_code() {
        let json = r#"{"type":"tap","code":"xoooo"}"#;
        let t: Trigger = serde_json::from_str(json).unwrap();
        assert_eq!(
            t,
            Trigger::Tap {
                code: tap_code(0b00001)
            }
        );
    }

    #[test]
    fn trigger_tap_deserialises_from_legacy_integer_code() {
        let json = r#"{"type":"tap","code":1}"#;
        let t: Trigger = serde_json::from_str(json).unwrap();
        assert_eq!(t, Trigger::Tap { code: tap_code(1) });
    }

    #[test]
    fn trigger_tap_deserialises_dual_string_code() {
        let json = r#"{"type":"tap","code":"oooox xoooo"}"#;
        let t: Trigger = serde_json::from_str(json).unwrap();
        // Both thumbs: left "oooox" (pinky-first) = thumb = bit0; right "xoooo" = thumb = bit0
        let expected = TriggerPattern::Dual {
            left: TapCode::from_u8(0b00001).unwrap(),
            right: TapCode::from_u8(0b00001).unwrap(),
        };
        assert_eq!(t, Trigger::Tap { code: expected });
    }

    #[test]
    fn trigger_sequence_deserialises_steps_from_strings() {
        let json = r#"{"type":"sequence","steps":["xoooo","ooxoo"],"window_ms":400}"#;
        let t: Trigger = serde_json::from_str(json).unwrap();
        assert_eq!(
            t,
            Trigger::Sequence {
                steps: vec![
                    TapStep {
                        code: tap_code(0b00001)
                    },
                    TapStep {
                        code: tap_code(0b00100)
                    },
                ],
                window_ms: Some(400),
            }
        );
    }
}
