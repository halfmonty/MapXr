use serde::{Deserialize, Serialize};

/// A validated key name string, as used in [`Action::Key`](crate::types::Action::Key)
/// and [`Action::KeyChord`](crate::types::Action::KeyChord).
///
/// Serialises and deserialises as a plain JSON string. Validation against the
/// known key name list is performed separately via [`KeyDef::validate`]; profile
/// loading calls this after deserialising the full profile.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyDef(String);

impl KeyDef {
    /// Wrap a string as a `KeyDef` without validating it.
    ///
    /// Prefer constructing through deserialisation; use this in tests.
    pub fn new_unchecked(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Return the key name string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check whether this key name is in the valid key list.
    ///
    /// Returns `Ok(())` if valid, or a [`KeyDefError`] describing the unknown name.
    pub fn validate(&self) -> Result<(), KeyDefError> {
        if VALID_KEYS.binary_search(&self.0.as_str()).is_ok() {
            Ok(())
        } else {
            Err(KeyDefError::UnknownKey(self.0.clone()))
        }
    }
}

impl std::fmt::Display for KeyDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error returned by [`KeyDef::validate`] for unknown key names.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyDefError {
    /// The key name is not in the valid key list.
    #[error(
        "unknown key name {0:?}; valid names include a–z, 0–9, f1–f24, \
         space, return, escape, tab, and named keys — see the key name reference"
    )]
    UnknownKey(String),
}

/// Sorted list of all valid key name strings.
///
/// Must remain sorted for [`binary_search`](slice::binary_search) to work.
/// See `docs/spec/extended-keys-spec.md` for the complete platform availability matrix.
pub const VALID_KEYS: &[&str] = &[
    // ── Digits ────────────────────────────────────────────────────────────────
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    // ── Letters ───────────────────────────────────────────────────────────────
    "a",
    "b",
    // ── Punctuation ───────────────────────────────────────────────────────────
    "backslash",
    // ── Navigation ────────────────────────────────────────────────────────────
    "backspace",
    // ── System (platform-limited) ─────────────────────────────────────────────
    "brightness_down",
    "brightness_up",
    // ── Letters ───────────────────────────────────────────────────────────────
    "c",
    // ── Navigation ────────────────────────────────────────────────────────────
    "caps_lock",
    // ── Punctuation ───────────────────────────────────────────────────────────
    "comma",
    // ── Letters ───────────────────────────────────────────────────────────────
    "d",
    // ── Navigation ────────────────────────────────────────────────────────────
    "delete",
    "down_arrow",
    // ── Letters ───────────────────────────────────────────────────────────────
    "e",
    // ── System (platform-limited) ─────────────────────────────────────────────
    "eject",
    // ── Navigation ────────────────────────────────────────────────────────────
    "end",
    // ── Punctuation ───────────────────────────────────────────────────────────
    "equals",
    // ── Navigation ────────────────────────────────────────────────────────────
    "escape",
    // ── Letters ───────────────────────────────────────────────────────────────
    "f",
    // ── Function keys ─────────────────────────────────────────────────────────
    "f1",
    "f10",
    "f11",
    "f12",
    "f13",
    "f14",
    "f15",
    "f16",
    "f17",
    "f18",
    "f19",
    "f2",
    "f20",
    "f21",
    "f22",
    "f23",
    "f24",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    // ── Letters ───────────────────────────────────────────────────────────────
    "g",
    // ── Punctuation ───────────────────────────────────────────────────────────
    "grave",
    // ── Letters ───────────────────────────────────────────────────────────────
    "h",
    // ── Navigation ────────────────────────────────────────────────────────────
    "home",
    // ── Letters ───────────────────────────────────────────────────────────────
    "i",
    // ── Navigation ────────────────────────────────────────────────────────────
    "insert",
    // ── Letters ───────────────────────────────────────────────────────────────
    "j",
    "k",
    "l",
    // ── Navigation ────────────────────────────────────────────────────────────
    "left_arrow",
    // ── Punctuation ───────────────────────────────────────────────────────────
    "left_bracket",
    // ── Letters ───────────────────────────────────────────────────────────────
    "m",
    // ── Media ─────────────────────────────────────────────────────────────────
    "media_next",
    "media_play",
    "media_prev",
    "media_stop",
    // ── System (platform-limited) ─────────────────────────────────────────────
    "mic_mute",
    // ── Punctuation ───────────────────────────────────────────────────────────
    "minus",
    // ── Letters ───────────────────────────────────────────────────────────────
    "n",
    // ── Navigation ────────────────────────────────────────────────────────────
    "num_lock",
    // ── Letters ───────────────────────────────────────────────────────────────
    "o",
    "p",
    // ── Navigation ────────────────────────────────────────────────────────────
    "page_down",
    "page_up",
    // ── System (platform-limited) ─────────────────────────────────────────────
    "pause",
    // ── Punctuation ───────────────────────────────────────────────────────────
    "period",
    // ── Navigation ────────────────────────────────────────────────────────────
    "print_screen",
    // ── Letters ───────────────────────────────────────────────────────────────
    "q",
    // ── Punctuation ───────────────────────────────────────────────────────────
    "quote",
    // ── Letters ───────────────────────────────────────────────────────────────
    "r",
    // ── Navigation ────────────────────────────────────────────────────────────
    "return",
    "right_arrow",
    // ── Punctuation ───────────────────────────────────────────────────────────
    "right_bracket",
    // ── Letters ───────────────────────────────────────────────────────────────
    "s",
    // ── Navigation ────────────────────────────────────────────────────────────
    "scroll_lock",
    // ── Punctuation ───────────────────────────────────────────────────────────
    "semicolon",
    "slash",
    // ── Navigation ────────────────────────────────────────────────────────────
    "space",
    // ── Letters ───────────────────────────────────────────────────────────────
    "t",
    // ── Navigation ────────────────────────────────────────────────────────────
    "tab",
    // ── Letters ───────────────────────────────────────────────────────────────
    "u",
    // ── Navigation ────────────────────────────────────────────────────────────
    "up_arrow",
    // ── Letters ───────────────────────────────────────────────────────────────
    "v",
    // ── Volume ────────────────────────────────────────────────────────────────
    "volume_down",
    "volume_mute",
    "volume_up",
    // ── Letters ───────────────────────────────────────────────────────────────
    "w",
    "x",
    "y",
    "z",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_keys_slice_is_sorted() {
        let mut sorted = VALID_KEYS.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            VALID_KEYS,
            sorted.as_slice(),
            "VALID_KEYS must be sorted for binary_search"
        );
    }

    #[test]
    fn key_def_validate_accepts_all_listed_keys() {
        for key in VALID_KEYS {
            let kd = KeyDef::new_unchecked(*key);
            assert!(kd.validate().is_ok(), "expected {key:?} to be valid");
        }
    }

    #[test]
    fn key_def_validate_rejects_unknown_key() {
        let kd = KeyDef::new_unchecked("unknown_key");
        assert!(matches!(kd.validate(), Err(KeyDefError::UnknownKey(_))));
    }

    #[test]
    fn key_def_validate_rejects_empty_string() {
        let kd = KeyDef::new_unchecked("");
        assert!(matches!(kd.validate(), Err(KeyDefError::UnknownKey(_))));
    }

    #[test]
    fn key_def_round_trips_via_serde() {
        let kd = KeyDef::new_unchecked("space");
        let json = serde_json::to_string(&kd).unwrap();
        assert_eq!(json, r#""space""#);
        let parsed: KeyDef = serde_json::from_str(&json).unwrap();
        assert_eq!(kd, parsed);
    }

    // ── New keys added in Epic 17 ─────────────────────────────────────────────

    #[test]
    fn key_def_validate_accepts_arrow_keys_with_suffix() {
        for key in ["left_arrow", "right_arrow", "up_arrow", "down_arrow"] {
            let kd = KeyDef::new_unchecked(key);
            assert!(kd.validate().is_ok(), "expected {key:?} to be valid");
        }
    }

    #[test]
    fn key_def_validate_accepts_f_keys_lowercase() {
        for n in 1u8..=24 {
            let name = format!("f{n}");
            let kd = KeyDef::new_unchecked(&name);
            assert!(kd.validate().is_ok(), "expected {name:?} to be valid");
        }
    }

    #[test]
    fn key_def_validate_accepts_punctuation_keys() {
        for key in [
            "grave", "minus", "equals", "left_bracket", "right_bracket",
            "backslash", "semicolon", "quote", "comma", "period", "slash",
        ] {
            let kd = KeyDef::new_unchecked(key);
            assert!(kd.validate().is_ok(), "expected {key:?} to be valid");
        }
    }

    #[test]
    fn key_def_validate_accepts_media_and_volume_keys() {
        for key in [
            "media_play", "media_next", "media_prev", "media_stop",
            "volume_up", "volume_down", "volume_mute",
        ] {
            let kd = KeyDef::new_unchecked(key);
            assert!(kd.validate().is_ok(), "expected {key:?} to be valid");
        }
    }

    #[test]
    fn key_def_validate_accepts_system_keys() {
        for key in [
            "caps_lock", "insert", "num_lock", "scroll_lock", "print_screen",
            "pause", "brightness_down", "brightness_up", "eject", "mic_mute",
        ] {
            let kd = KeyDef::new_unchecked(key);
            assert!(kd.validate().is_ok(), "expected {key:?} to be valid");
        }
    }

    #[test]
    fn key_def_validate_rejects_old_broken_arrow_names() {
        // These were the incorrect names present in the old pump.rs (bug 1).
        for key in ["left", "right", "up", "down"] {
            let kd = KeyDef::new_unchecked(key);
            assert!(
                kd.validate().is_err(),
                "expected old arrow name {key:?} to be rejected"
            );
        }
    }

    #[test]
    fn key_def_validate_rejects_uppercase_f_key_names() {
        // These were the incorrect names present in the old pump.rs (bug 2).
        for key in ["F1", "F2", "F12"] {
            let kd = KeyDef::new_unchecked(key);
            assert!(
                kd.validate().is_err(),
                "expected uppercase F-key name {key:?} to be rejected"
            );
        }
    }

    #[test]
    fn key_def_f_key_round_trips_via_serde() {
        for n in [1u8, 12, 13, 20, 24] {
            let name = format!("f{n}");
            let kd = KeyDef::new_unchecked(&name);
            let json = serde_json::to_string(&kd).expect("serialize");
            let parsed: KeyDef = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(kd, parsed, "round-trip failed for {name:?}");
        }
    }

    #[test]
    fn key_def_media_stop_round_trips_via_serde() {
        let kd = KeyDef::new_unchecked("media_stop");
        let json = serde_json::to_string(&kd).expect("serialize");
        assert_eq!(json, r#""media_stop""#);
        let parsed: KeyDef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(kd, parsed);
    }
}
