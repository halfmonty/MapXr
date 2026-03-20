use serde::{Deserialize, Serialize};

/// The direction of a [`crate::types::Action::MouseScroll`] action.
///
/// Serialises as a lowercase string: `"up"`, `"down"`, `"left"`, `"right"`.
/// Unknown values are rejected at deserialisation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    /// Scroll upward (toward the top of the page).
    Up,
    /// Scroll downward (toward the bottom of the page).
    Down,
    /// Scroll left.
    Left,
    /// Scroll right.
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(d: ScrollDirection) -> ScrollDirection {
        let json = serde_json::to_string(&d).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn scroll_direction_up_serialises_as_lowercase_string() {
        let json = serde_json::to_string(&ScrollDirection::Up).expect("serialize");
        assert_eq!(json, r#""up""#);
    }

    #[test]
    fn scroll_direction_down_serialises_as_lowercase_string() {
        let json = serde_json::to_string(&ScrollDirection::Down).expect("serialize");
        assert_eq!(json, r#""down""#);
    }

    #[test]
    fn scroll_direction_left_serialises_as_lowercase_string() {
        let json = serde_json::to_string(&ScrollDirection::Left).expect("serialize");
        assert_eq!(json, r#""left""#);
    }

    #[test]
    fn scroll_direction_right_serialises_as_lowercase_string() {
        let json = serde_json::to_string(&ScrollDirection::Right).expect("serialize");
        assert_eq!(json, r#""right""#);
    }

    #[test]
    fn scroll_direction_all_variants_round_trip() {
        for d in [
            ScrollDirection::Up,
            ScrollDirection::Down,
            ScrollDirection::Left,
            ScrollDirection::Right,
        ] {
            assert_eq!(round_trip(d), d);
        }
    }

    #[test]
    fn scroll_direction_unknown_string_returns_error() {
        let result: Result<ScrollDirection, _> = serde_json::from_str(r#""diagonal""#);
        assert!(result.is_err(), "expected error for unknown direction");
    }
}
