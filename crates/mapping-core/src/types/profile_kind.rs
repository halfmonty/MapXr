use serde::{Deserialize, Serialize};

/// Whether a profile targets one or two Tap devices.
///
/// Serialises as a lowercase JSON string matching the profile `"kind"` field:
///
/// ```json
/// { "kind": "single" }
/// { "kind": "dual" }
/// ```
///
/// The engine checks `kind` against the number of connected devices at load time
/// and warns if they do not match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfileKind {
    /// Profile targets a single Tap device. Trigger codes are 5-char strings.
    /// A top-level `hand` field declares which hand the device is worn on.
    Single,
    /// Profile targets two Tap devices (left + right). Trigger codes are
    /// 11-char `"ooooo ooooo"` dual strings.
    Dual,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_kind_single_serialises_as_lowercase() {
        let json = serde_json::to_string(&ProfileKind::Single).unwrap();
        assert_eq!(json, r#""single""#);
    }

    #[test]
    fn profile_kind_dual_serialises_as_lowercase() {
        let json = serde_json::to_string(&ProfileKind::Dual).unwrap();
        assert_eq!(json, r#""dual""#);
    }

    #[test]
    fn profile_kind_single_round_trips() {
        let json = serde_json::to_string(&ProfileKind::Single).unwrap();
        let parsed: ProfileKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ProfileKind::Single);
    }

    #[test]
    fn profile_kind_dual_round_trips() {
        let json = serde_json::to_string(&ProfileKind::Dual).unwrap();
        let parsed: ProfileKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ProfileKind::Dual);
    }

    #[test]
    fn profile_kind_deserialises_from_profile_json_single() {
        let parsed: ProfileKind = serde_json::from_str(r#""single""#).unwrap();
        assert_eq!(parsed, ProfileKind::Single);
    }

    #[test]
    fn profile_kind_deserialises_from_profile_json_dual() {
        let parsed: ProfileKind = serde_json::from_str(r#""dual""#).unwrap();
        assert_eq!(parsed, ProfileKind::Dual);
    }
}
