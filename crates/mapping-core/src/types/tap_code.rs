use serde::{
    Deserializer, Serializer,
    de::{self, Visitor},
};

use crate::types::Hand;

/// A raw tap code from the hardware, represented as a bitmask over the five fingers.
///
/// The hardware normalises bit positions to physical fingers regardless of which hand
/// the device is worn on: bit 0 = thumb, bit 1 = index, bit 2 = middle, bit 3 = ring,
/// bit 4 = pinky. [`Hand`] context is only needed when parsing or serialising the
/// finger-pattern string notation.
///
/// Valid codes are in the range `0–31`. The value `0` (`ooooo`) is valid at the type
/// level (it appears as the idle side in dual patterns), but is rejected as a standalone
/// trigger by profile validation.
///
/// # Serialisation
///
/// - **Deserialise**: accepts a legacy `u8` integer directly. Single-hand and dual
///   finger-pattern strings require [`Hand`] context and are parsed via
///   [`TapCode::from_single_pattern`] / [`TriggerPattern::from_dual_pattern`],
///   which the profile loader calls after the `hand` field is known.
/// - **Serialise**: always writes the canonical right-hand string form. Profile
///   serialisation calls [`TapCode::to_single_pattern`] with the correct [`Hand`]
///   instead of relying on this trait impl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TapCode(u8);

impl TapCode {
    /// The maximum valid raw value (five fingers → 2⁵ − 1).
    pub const MAX: u8 = 31;

    /// Construct a `TapCode` from a raw `u8`, returning `None` if `raw > 31`.
    pub fn from_u8(raw: u8) -> Option<Self> {
        if raw <= Self::MAX {
            Some(Self(raw))
        } else {
            None
        }
    }

    /// Return the underlying raw `u8` value.
    pub fn as_u8(self) -> u8 {
        self.0
    }

    /// Parse a 5-character finger-pattern string into a `TapCode`.
    ///
    /// The string must contain exactly 5 characters, each `'o'` (not tapped) or
    /// `'x'` (tapped), case-insensitive. Canonical form uses lowercase.
    ///
    /// Read direction depends on `hand`:
    /// - [`Hand::Right`]: position 0 = thumb (bit 0), position 4 = pinky (bit 4).
    /// - [`Hand::Left`]: position 0 = pinky (bit 4), position 4 = thumb (bit 0).
    pub fn from_single_pattern(s: &str, hand: Hand) -> Result<Self, TapCodeError> {
        let lower = s.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();

        if chars.len() != 5 {
            return Err(TapCodeError::InvalidLength {
                input: s.to_owned(),
                len: chars.len(),
            });
        }

        for &ch in &chars {
            if ch != 'o' && ch != 'x' {
                return Err(TapCodeError::InvalidChar {
                    ch,
                    input: s.to_owned(),
                });
            }
        }

        let value: u8 = chars
            .iter()
            .enumerate()
            .filter(|(_, c)| **c == 'x')
            .map(|(i, _)| match hand {
                Hand::Right => 1u8 << i,
                Hand::Left => 1u8 << (4 - i),
            })
            .sum();

        Ok(Self(value))
    }

    /// Format this code as a 5-character finger-pattern string.
    ///
    /// The write direction mirrors [`from_single_pattern`](Self::from_single_pattern):
    /// right-hand writes thumb at position 0; left-hand writes pinky at position 0.
    pub fn to_single_pattern(self, hand: Hand) -> String {
        (0..5u8)
            .map(|i| {
                let bit = match hand {
                    Hand::Right => 1u8 << i,
                    Hand::Left => 1u8 << (4 - i),
                };
                if self.0 & bit != 0 { 'x' } else { 'o' }
            })
            .collect()
    }

    /// Decode the code into named boolean fields, one per finger.
    ///
    /// Bit assignments are fixed by the hardware regardless of hand orientation:
    /// bit 0 = thumb, bit 1 = index, bit 2 = middle, bit 3 = ring, bit 4 = pinky.
    pub fn fingers(self) -> Fingers {
        let v = self.0;
        Fingers {
            thumb: v & 0b00001 != 0,
            index: v & 0b00010 != 0,
            middle: v & 0b00100 != 0,
            ring: v & 0b01000 != 0,
            pinky: v & 0b10000 != 0,
        }
    }
}

// Custom Deserialize: accepts legacy u8 integers only.
// String-form codes require Hand context and are parsed by the profile loader.
impl<'de> serde::Deserialize<'de> for TapCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TapCodeVisitor;

        impl<'de> Visitor<'de> for TapCodeVisitor {
            type Value = TapCode;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "an integer tap code in the range 0–31")
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<TapCode, E> {
                let raw = u8::try_from(v)
                    .ok()
                    .filter(|&r| r <= TapCode::MAX)
                    .ok_or_else(|| E::custom(format!("tap code {v} out of range; must be 0–31")))?;
                Ok(TapCode(raw))
            }
        }

        deserializer.deserialize_u64(TapCodeVisitor)
    }
}

// Custom Serialize: always writes the canonical right-hand string form.
// Profile serialisation uses to_single_pattern(hand) directly.
impl serde::Serialize for TapCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_single_pattern(Hand::Right))
    }
}

/// Error type for finger-pattern parsing failures.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TapCodeError {
    /// The string was not exactly 5 characters long.
    #[error("finger pattern must be exactly 5 characters, got {len} in {input:?}")]
    InvalidLength { input: String, len: usize },

    /// The string contained a character other than `'o'` or `'x'`.
    #[error(
        "finger pattern contains invalid character {ch:?} in {input:?}; only 'o' and 'x' are allowed"
    )]
    InvalidChar { ch: char, input: String },

    /// A dual pattern string was malformed (not two 5-char groups separated by a space).
    #[error("dual pattern must be two 5-character groups separated by a single space, got {0:?}")]
    InvalidDualFormat(String),

    /// A raw integer was outside the valid range 0–31.
    #[error("integer tap code {0} is out of range; must be 0–31")]
    OutOfRange(u8),
}

/// The five fingers decoded from a [`TapCode`].
///
/// Bit-to-finger mapping is fixed at the hardware level: thumb=bit0, index=bit1,
/// middle=bit2, ring=bit3, pinky=bit4, regardless of which hand wears the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingers {
    /// Thumb (bit 0).
    pub thumb: bool,
    /// Index finger (bit 1).
    pub index: bool,
    /// Middle finger (bit 2).
    pub middle: bool,
    /// Ring finger (bit 3).
    pub ring: bool,
    /// Pinky (bit 4).
    pub pinky: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── from_u8 ──────────────────────────────────────────────────────────────

    #[test]
    fn tap_code_from_u8_valid_range_succeeds() {
        for v in 0u8..=31 {
            assert!(TapCode::from_u8(v).is_some(), "expected Some for {v}");
        }
    }

    #[test]
    fn tap_code_from_u8_above_max_returns_none() {
        for v in 32u8..=255 {
            assert!(TapCode::from_u8(v).is_none(), "expected None for {v}");
        }
    }

    #[test]
    fn tap_code_as_u8_round_trips() {
        for v in 0u8..=31 {
            assert_eq!(TapCode::from_u8(v).unwrap().as_u8(), v);
        }
    }

    // ── from_single_pattern ───────────────────────────────────────────────────

    #[test]
    fn tap_code_from_single_pattern_right_hand_thumb_first() {
        // Right hand: position 0 = thumb
        let code = TapCode::from_single_pattern("xoooo", Hand::Right).unwrap();
        assert_eq!(code.as_u8(), 0b00001, "thumb = bit 0");

        let code = TapCode::from_single_pattern("oooox", Hand::Right).unwrap();
        assert_eq!(code.as_u8(), 0b10000, "pinky = bit 4");

        let code = TapCode::from_single_pattern("ooxoo", Hand::Right).unwrap();
        assert_eq!(code.as_u8(), 0b00100, "middle = bit 2");
    }

    #[test]
    fn tap_code_from_single_pattern_left_hand_pinky_first() {
        // Left hand: position 0 = pinky, position 4 = thumb
        let code = TapCode::from_single_pattern("xoooo", Hand::Left).unwrap();
        assert_eq!(code.as_u8(), 0b10000, "pinky = bit 4");

        let code = TapCode::from_single_pattern("oooox", Hand::Left).unwrap();
        assert_eq!(code.as_u8(), 0b00001, "thumb = bit 0");

        let code = TapCode::from_single_pattern("ooxoo", Hand::Left).unwrap();
        assert_eq!(code.as_u8(), 0b00100, "middle = bit 2");
    }

    #[test]
    fn tap_code_from_single_pattern_case_insensitive() {
        let lower = TapCode::from_single_pattern("xoooo", Hand::Right).unwrap();
        let upper = TapCode::from_single_pattern("XOOOO", Hand::Right).unwrap();
        let mixed = TapCode::from_single_pattern("XoOoO", Hand::Right).unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower, mixed);
    }

    #[test]
    fn tap_code_from_single_pattern_symmetric_hands() {
        // "oooox" left == "xoooo" right (both = thumb only)
        let left_thumb = TapCode::from_single_pattern("oooox", Hand::Left).unwrap();
        let right_thumb = TapCode::from_single_pattern("xoooo", Hand::Right).unwrap();
        assert_eq!(left_thumb, right_thumb);
    }

    #[test]
    fn tap_code_from_single_pattern_wrong_length_returns_error() {
        assert!(matches!(
            TapCode::from_single_pattern("xooo", Hand::Right),
            Err(TapCodeError::InvalidLength { len: 4, .. })
        ));
        assert!(matches!(
            TapCode::from_single_pattern("xooooo", Hand::Right),
            Err(TapCodeError::InvalidLength { len: 6, .. })
        ));
        assert!(matches!(
            TapCode::from_single_pattern("", Hand::Right),
            Err(TapCodeError::InvalidLength { len: 0, .. })
        ));
    }

    #[test]
    fn tap_code_from_single_pattern_invalid_char_returns_error() {
        assert!(matches!(
            TapCode::from_single_pattern("xooxz", Hand::Right),
            Err(TapCodeError::InvalidChar { ch: 'z', .. })
        ));
        assert!(matches!(
            TapCode::from_single_pattern("xoo1o", Hand::Right),
            Err(TapCodeError::InvalidChar { ch: '1', .. })
        ));
    }

    // ── to_single_pattern ────────────────────────────────────────────────────

    #[test]
    fn tap_code_to_single_pattern_right_hand_round_trips() {
        for s in ["xoooo", "oooox", "ooxoo", "xxxxx", "ooooo"] {
            let code = TapCode::from_single_pattern(s, Hand::Right).unwrap();
            assert_eq!(code.to_single_pattern(Hand::Right), s);
        }
    }

    #[test]
    fn tap_code_to_single_pattern_left_hand_round_trips() {
        for s in ["xoooo", "oooox", "ooxoo", "xxxxx", "ooooo"] {
            let code = TapCode::from_single_pattern(s, Hand::Left).unwrap();
            assert_eq!(code.to_single_pattern(Hand::Left), s);
        }
    }

    #[test]
    fn tap_code_to_single_pattern_cross_hand_mirrors() {
        // "oooox" right-hand-parsed serialises as "xoooo" left-hand and vice versa
        let code = TapCode::from_single_pattern("oooox", Hand::Right).unwrap();
        assert_eq!(code.to_single_pattern(Hand::Left), "xoooo");

        let code = TapCode::from_single_pattern("xoooo", Hand::Left).unwrap();
        assert_eq!(code.to_single_pattern(Hand::Right), "oooox");
    }

    // ── serde ─────────────────────────────────────────────────────────────────

    #[test]
    fn tap_code_deserialize_from_u8_succeeds() {
        let code: TapCode = serde_json::from_str("4").unwrap();
        assert_eq!(code.as_u8(), 4);
    }

    #[test]
    fn tap_code_deserialize_from_u8_out_of_range_returns_error() {
        assert!(serde_json::from_str::<TapCode>("32").is_err());
        assert!(serde_json::from_str::<TapCode>("255").is_err());
    }

    #[test]
    fn tap_code_serialize_writes_canonical_right_hand_string() {
        let code = TapCode::from_u8(0b00001).unwrap(); // thumb
        let s = serde_json::to_string(&code).unwrap();
        assert_eq!(s, r#""xoooo""#);

        let code = TapCode::from_u8(0b10000).unwrap(); // pinky
        let s = serde_json::to_string(&code).unwrap();
        assert_eq!(s, r#""oooox""#);
    }

    // ── fingers ───────────────────────────────────────────────────────────────

    #[test]
    fn tap_code_fingers_all_off_returns_all_false() {
        let f = TapCode::from_u8(0b00000).unwrap().fingers();
        assert!(!f.thumb && !f.index && !f.middle && !f.ring && !f.pinky);
    }

    #[test]
    fn tap_code_fingers_all_on_returns_all_true() {
        let f = TapCode::from_u8(0b11111).unwrap().fingers();
        assert!(f.thumb && f.index && f.middle && f.ring && f.pinky);
    }

    #[test]
    fn tap_code_fingers_individual_bits_map_to_correct_finger() {
        let thumb = TapCode::from_u8(0b00001).unwrap().fingers();
        assert!(thumb.thumb && !thumb.index && !thumb.middle && !thumb.ring && !thumb.pinky);

        let index = TapCode::from_u8(0b00010).unwrap().fingers();
        assert!(!index.thumb && index.index && !index.middle && !index.ring && !index.pinky);

        let middle = TapCode::from_u8(0b00100).unwrap().fingers();
        assert!(!middle.thumb && !middle.index && middle.middle && !middle.ring && !middle.pinky);

        let ring = TapCode::from_u8(0b01000).unwrap().fingers();
        assert!(!ring.thumb && !ring.index && !ring.middle && ring.ring && !ring.pinky);

        let pinky = TapCode::from_u8(0b10000).unwrap().fingers();
        assert!(!pinky.thumb && !pinky.index && !pinky.middle && !pinky.ring && pinky.pinky);
    }
}
