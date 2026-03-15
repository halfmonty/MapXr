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
pub const VALID_KEYS: &[&str] = &[
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
    "a",
    "b",
    "backslash",
    "backspace",
    "c",
    "caps_lock",
    "comma",
    "d",
    "delete",
    "down_arrow",
    "e",
    "end",
    "equals",
    "escape",
    "f",
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
    "g",
    "grave",
    "h",
    "home",
    "i",
    "insert",
    "j",
    "k",
    "l",
    "left_arrow",
    "left_bracket",
    "m",
    "media_next",
    "media_play",
    "media_prev",
    "minus",
    "n",
    "num_lock",
    "o",
    "p",
    "page_down",
    "page_up",
    "period",
    "print_screen",
    "q",
    "quote",
    "r",
    "return",
    "right_arrow",
    "right_bracket",
    "s",
    "scroll_lock",
    "semicolon",
    "slash",
    "space",
    "t",
    "tab",
    "u",
    "up_arrow",
    "v",
    "volume_down",
    "volume_mute",
    "volume_up",
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
}
