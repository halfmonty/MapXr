use serde::{Deserialize, Serialize};

/// How the engine resolves a tap code that appears in both a `tap` and a
/// `double_tap` (or `triple_tap`) binding.
///
/// Serialises as a lowercase JSON string in the profile `settings` object:
///
/// ```json
/// { "overload_strategy": "patient" }
/// { "overload_strategy": "eager" }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverloadStrategy {
    /// Wait `double_tap_window_ms` before firing anything.
    ///
    /// Adds latency to every single-tap on overloaded codes. No visible
    /// artifact. Ideal when the double-tap action is destructive (e.g. delete,
    /// cut).
    Patient,
    /// Fire the single-tap action immediately.
    ///
    /// If a double-tap is then detected, send the configured
    /// `eager_undo_sequence` and fire the double-tap action instead. Adds no
    /// latency but may produce a brief visible artifact.
    Eager,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overload_strategy_patient_serialises_as_lowercase() {
        let json = serde_json::to_string(&OverloadStrategy::Patient).unwrap();
        assert_eq!(json, r#""patient""#);
    }

    #[test]
    fn overload_strategy_eager_serialises_as_lowercase() {
        let json = serde_json::to_string(&OverloadStrategy::Eager).unwrap();
        assert_eq!(json, r#""eager""#);
    }

    #[test]
    fn overload_strategy_patient_round_trips() {
        let json = serde_json::to_string(&OverloadStrategy::Patient).unwrap();
        let parsed: OverloadStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, OverloadStrategy::Patient);
    }

    #[test]
    fn overload_strategy_eager_round_trips() {
        let json = serde_json::to_string(&OverloadStrategy::Eager).unwrap();
        let parsed: OverloadStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, OverloadStrategy::Eager);
    }
}
