use serde::{Deserialize, Serialize};

/// A mouse button used in click actions.
///
/// Serialises as a lowercase string: `"left"`, `"right"`, `"middle"`.
/// Unknown values are rejected at deserialisation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    /// The primary (left) mouse button.
    Left,
    /// The secondary (right) mouse button.
    Right,
    /// The scroll-wheel (middle) mouse button.
    Middle,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(b: MouseButton) -> MouseButton {
        let json = serde_json::to_string(&b).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn mouse_button_left_serialises_as_lowercase_string() {
        let json = serde_json::to_string(&MouseButton::Left).expect("serialize");
        assert_eq!(json, r#""left""#);
    }

    #[test]
    fn mouse_button_right_serialises_as_lowercase_string() {
        let json = serde_json::to_string(&MouseButton::Right).expect("serialize");
        assert_eq!(json, r#""right""#);
    }

    #[test]
    fn mouse_button_middle_serialises_as_lowercase_string() {
        let json = serde_json::to_string(&MouseButton::Middle).expect("serialize");
        assert_eq!(json, r#""middle""#);
    }

    #[test]
    fn mouse_button_left_round_trips() {
        assert_eq!(round_trip(MouseButton::Left), MouseButton::Left);
    }

    #[test]
    fn mouse_button_right_round_trips() {
        assert_eq!(round_trip(MouseButton::Right), MouseButton::Right);
    }

    #[test]
    fn mouse_button_middle_round_trips() {
        assert_eq!(round_trip(MouseButton::Middle), MouseButton::Middle);
    }

    #[test]
    fn mouse_button_unknown_string_returns_error() {
        let result: Result<MouseButton, _> = serde_json::from_str(r#""thumb""#);
        assert!(result.is_err(), "expected error for unknown button name");
    }
}
