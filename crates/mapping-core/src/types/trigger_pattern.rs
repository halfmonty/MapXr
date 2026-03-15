use serde::{
    Deserializer, Serializer,
    de::{self, Visitor},
};

use crate::types::{Hand, TapCode, TapCodeError};

/// The finger-pattern code for a trigger, covering both single-device and
/// cross-device (dual) configurations.
///
/// In a `single`-kind profile, every trigger uses [`TriggerPattern::Single`].
/// In a `dual`-kind profile, triggers may specify one or both hands.
///
/// # Serialisation
///
/// The canonical serialised form is always a finger-pattern string, never an
/// integer. Use [`TriggerPattern::to_pattern_string`] for serialisation with
/// hand context. Profile loading uses [`TriggerPattern::from_dual_pattern`] or
/// [`TapCode::from_single_pattern`] depending on profile kind.
///
/// # Validation
///
/// The all-open patterns (`"ooooo"` as a single, `"ooooo ooooo"` as a dual) are
/// structurally valid at this type level but are rejected as standalone trigger
/// codes by profile validation (see `Profile::load`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerPattern {
    /// A single-device tap code. Used in `single`-kind profiles.
    Single(TapCode),
    /// A cross-device chord: both devices must fire within the combo window.
    /// Used in `dual`-kind profiles. `left` is the left-hand device code,
    /// `right` is the right-hand device code.
    Dual { left: TapCode, right: TapCode },
}

impl TriggerPattern {
    /// Parse an 11-character dual finger-pattern string of the form `"ooooo ooooo"`.
    ///
    /// The left group is always pinky-first ([`Hand::Left`]); the right group is
    /// always thumb-first ([`Hand::Right`]). The space separator is required.
    ///
    /// Returns [`TapCodeError::InvalidDualFormat`] if the string is not in the
    /// expected format, or a [`TapCodeError`] variant if either half contains
    /// invalid characters or lengths.
    pub fn from_dual_pattern(s: &str) -> Result<Self, TapCodeError> {
        let lower = s.to_lowercase();
        let mut parts = lower.splitn(2, ' ');

        let left_str = parts
            .next()
            .ok_or_else(|| TapCodeError::InvalidDualFormat(s.to_owned()))?;
        let right_str = parts
            .next()
            .ok_or_else(|| TapCodeError::InvalidDualFormat(s.to_owned()))?;

        // Ensure there are no additional spaces (e.g. "ooooo ooooo ooooo")
        if right_str.contains(' ') {
            return Err(TapCodeError::InvalidDualFormat(s.to_owned()));
        }

        if left_str.len() != 5 || right_str.len() != 5 {
            return Err(TapCodeError::InvalidDualFormat(s.to_owned()));
        }

        let left = TapCode::from_single_pattern(left_str, Hand::Left)?;
        let right = TapCode::from_single_pattern(right_str, Hand::Right)?;

        Ok(TriggerPattern::Dual { left, right })
    }

    /// Serialise this pattern to a finger-pattern string.
    ///
    /// - `Single`: uses `hand` to determine the string read direction.
    /// - `Dual`: always writes `"<left_pinky_first> <right_thumb_first>"`;
    ///   `hand` is ignored.
    pub fn to_pattern_string(self, hand: Hand) -> String {
        match self {
            TriggerPattern::Single(code) => code.to_single_pattern(hand),
            TriggerPattern::Dual { left, right } => format!(
                "{} {}",
                left.to_single_pattern(Hand::Left),
                right.to_single_pattern(Hand::Right)
            ),
        }
    }

    /// Returns `true` if this pattern is all-open (no fingers tapped).
    ///
    /// `"ooooo"` and `"ooooo ooooo"` are invalid as standalone trigger codes.
    /// Profile validation calls this to reject such patterns.
    pub fn is_all_open(self) -> bool {
        match self {
            TriggerPattern::Single(code) => code.as_u8() == 0,
            TriggerPattern::Dual { left, right } => left.as_u8() == 0 && right.as_u8() == 0,
        }
    }
}

/// Serialises as a finger-pattern string.
///
/// `Single` codes use canonical right-hand (thumb-first) format. Profile
/// serialisation must call [`TriggerPattern::to_pattern_string`] with the
/// correct [`Hand`] directly when writing left-hand profiles.
impl serde::Serialize for TriggerPattern {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_pattern_string(Hand::Right))
    }
}

/// Deserialises from a finger-pattern string or legacy `u8` integer.
///
/// - `u8` → `Single(TapCode)`
/// - 11-char string with one space → `Dual` (left pinky-first, right thumb-first)
/// - 5-char string → `Single` using right-hand (thumb-first) as default
///
/// **Left-hand profiles:** single-hand codes written in left-hand notation
/// (e.g. `"oooox"` meaning thumb) must be re-resolved by the profile loader
/// via [`TapCode::from_single_pattern`] with [`Hand::Left`]. The right-hand
/// default here will produce an incorrect bit pattern for left-hand strings.
impl<'de> serde::Deserialize<'de> for TriggerPattern {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TriggerPatternVisitor;

        impl<'de> Visitor<'de> for TriggerPatternVisitor {
            type Value = TriggerPattern;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "a finger-pattern string (\"oooox\" or \"ooooo ooooo\") or a legacy u8 integer"
                )
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<TriggerPattern, E> {
                let raw = u8::try_from(v)
                    .ok()
                    .filter(|&r| r <= TapCode::MAX)
                    .ok_or_else(|| E::custom(format!("tap code {v} out of range; must be 0–31")))?;
                Ok(TriggerPattern::Single(TapCode::from_u8(raw).unwrap()))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<TriggerPattern, E> {
                if v.contains(' ') {
                    TriggerPattern::from_dual_pattern(v).map_err(E::custom)
                } else {
                    TapCode::from_single_pattern(v, Hand::Right)
                        .map(TriggerPattern::Single)
                        .map_err(E::custom)
                }
            }
        }

        deserializer.deserialize_any(TriggerPatternVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── from_dual_pattern ─────────────────────────────────────────────────────

    #[test]
    fn trigger_pattern_from_dual_both_thumbs_parses_correctly() {
        // Left hand: "oooox" = thumb (pinky-first, so position 4 = thumb = bit 0)
        // Right hand: "xoooo" = thumb (thumb-first, so position 0 = thumb = bit 0)
        let p = TriggerPattern::from_dual_pattern("oooox xoooo").unwrap();
        assert_eq!(
            p,
            TriggerPattern::Dual {
                left: TapCode::from_u8(0b00001).unwrap(),  // thumb
                right: TapCode::from_u8(0b00001).unwrap(), // thumb
            }
        );
    }

    #[test]
    fn trigger_pattern_from_dual_left_only_parses_correctly() {
        // Left middle finger; right hand idle
        let p = TriggerPattern::from_dual_pattern("ooxoo ooooo").unwrap();
        assert_eq!(
            p,
            TriggerPattern::Dual {
                left: TapCode::from_u8(0b00100).unwrap(),  // middle
                right: TapCode::from_u8(0b00000).unwrap(), // idle
            }
        );
    }

    #[test]
    fn trigger_pattern_from_dual_right_only_parses_correctly() {
        // Left hand idle; right middle finger
        let p = TriggerPattern::from_dual_pattern("ooooo ooxoo").unwrap();
        assert_eq!(
            p,
            TriggerPattern::Dual {
                left: TapCode::from_u8(0b00000).unwrap(),  // idle
                right: TapCode::from_u8(0b00100).unwrap(), // middle
            }
        );
    }

    #[test]
    fn trigger_pattern_from_dual_case_insensitive() {
        let lower = TriggerPattern::from_dual_pattern("oooox xoooo").unwrap();
        let upper = TriggerPattern::from_dual_pattern("OOOOX XOOOO").unwrap();
        assert_eq!(lower, upper);
    }

    #[test]
    fn trigger_pattern_from_dual_missing_space_returns_error() {
        assert!(matches!(
            TriggerPattern::from_dual_pattern("oooox"),
            Err(TapCodeError::InvalidDualFormat(_))
        ));
    }

    #[test]
    fn trigger_pattern_from_dual_wrong_length_returns_error() {
        assert!(matches!(
            TriggerPattern::from_dual_pattern("ooox xoooo"),
            Err(TapCodeError::InvalidDualFormat(_))
        ));
        assert!(matches!(
            TriggerPattern::from_dual_pattern("oooox xooo"),
            Err(TapCodeError::InvalidDualFormat(_))
        ));
    }

    #[test]
    fn trigger_pattern_from_dual_extra_space_returns_error() {
        assert!(matches!(
            TriggerPattern::from_dual_pattern("oooox xoooo ooooo"),
            Err(TapCodeError::InvalidDualFormat(_))
        ));
    }

    #[test]
    fn trigger_pattern_from_dual_invalid_char_returns_error() {
        assert!(matches!(
            TriggerPattern::from_dual_pattern("ooooz xoooo"),
            Err(TapCodeError::InvalidChar { ch: 'z', .. })
        ));
    }

    // ── to_pattern_string ─────────────────────────────────────────────────────

    #[test]
    fn trigger_pattern_single_right_hand_round_trips() {
        for s in ["xoooo", "oooox", "ooxoo", "xxxxx"] {
            let code = TapCode::from_single_pattern(s, Hand::Right).unwrap();
            let pattern = TriggerPattern::Single(code);
            assert_eq!(pattern.to_pattern_string(Hand::Right), s);
        }
    }

    #[test]
    fn trigger_pattern_single_left_hand_round_trips() {
        for s in ["xoooo", "oooox", "ooxoo", "xxxxx"] {
            let code = TapCode::from_single_pattern(s, Hand::Left).unwrap();
            let pattern = TriggerPattern::Single(code);
            assert_eq!(pattern.to_pattern_string(Hand::Left), s);
        }
    }

    #[test]
    fn trigger_pattern_dual_round_trips() {
        for s in ["oooox xoooo", "ooxoo ooooo", "ooooo ooxoo", "xxxxx xxxxx"] {
            let pattern = TriggerPattern::from_dual_pattern(s).unwrap();
            assert_eq!(pattern.to_pattern_string(Hand::Right), s);
        }
    }

    // ── is_all_open ───────────────────────────────────────────────────────────

    #[test]
    fn trigger_pattern_single_all_open_detected() {
        let p = TriggerPattern::Single(TapCode::from_u8(0).unwrap());
        assert!(p.is_all_open());
    }

    #[test]
    fn trigger_pattern_single_not_all_open() {
        let p = TriggerPattern::Single(TapCode::from_u8(1).unwrap());
        assert!(!p.is_all_open());
    }

    #[test]
    fn trigger_pattern_dual_all_open_detected() {
        let p = TriggerPattern::from_dual_pattern("ooooo ooooo").unwrap();
        assert!(p.is_all_open());
    }

    #[test]
    fn trigger_pattern_dual_one_side_open_not_all_open() {
        let p = TriggerPattern::from_dual_pattern("ooooo xoooo").unwrap();
        assert!(!p.is_all_open());
    }
}
