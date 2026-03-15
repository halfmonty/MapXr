use serde::{Deserialize, Serialize};

use crate::types::Action;

/// Structured timing metadata emitted by the engine when debug mode is
/// enabled. Attached to [`EngineOutput::debug`](crate::engine::EngineOutput::debug).
///
/// The three variants mirror the three event types shown in the spec debug
/// panel: `resolved`, `unmatched`, and `combo_timeout`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DebugEvent {
    /// A trigger was resolved to a matching mapping and an action was fired.
    Resolved {
        /// Finger-pattern string of the resolved trigger.
        pattern: String,
        /// Device that produced the trigger.
        device: String,
        /// Layer stack at the time of resolution, top first.
        layer_stack: Vec<String>,
        /// `layer_id` of the layer where the match was found.
        matched_layer: String,
        /// `label` of the mapping that was matched.
        matched_mapping: String,
        /// The action that was fired.
        action_fired: Action,
        /// How long (ms) the engine held the event before resolving it.
        waited_ms: u64,
        /// The timing window (ms) that governed this resolution — used by the UI
        /// to render a waited_ms / window_ms timing bar.
        window_ms: u64,
    },

    /// A trigger arrived but no matching binding was found in any layer.
    Unmatched {
        /// Finger-pattern string of the unmatched trigger.
        pattern: String,
        /// Device that produced the trigger.
        device: String,
        /// Ordered list of `layer_id` values checked via passthrough walk.
        passthrough_layers_checked: Vec<String>,
    },

    /// Two pending events from different devices could not be matched as a
    /// cross-device combo because they arrived outside the combo window.
    ComboTimeout {
        /// Finger-pattern string of the first pending event.
        first_pattern: String,
        /// Device of the first pending event.
        first_device: String,
        /// Finger-pattern string of the second pending event.
        second_pattern: String,
        /// Device of the second pending event.
        second_device: String,
        /// The configured combo window in milliseconds.
        combo_window_ms: u64,
        /// The actual gap between the two events in milliseconds.
        actual_gap_ms: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_event_unmatched_stores_fields() {
        let ev = DebugEvent::Unmatched {
            pattern: "xoooo".into(),
            device: "solo".into(),
            passthrough_layers_checked: vec!["base".into()],
        };
        if let DebugEvent::Unmatched {
            pattern,
            device,
            passthrough_layers_checked,
        } = ev
        {
            assert_eq!(pattern, "xoooo");
            assert_eq!(device, "solo");
            assert_eq!(passthrough_layers_checked, vec!["base"]);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn debug_event_combo_timeout_stores_gap() {
        let ev = DebugEvent::ComboTimeout {
            first_pattern: "xoooo ooooo".into(),
            first_device: "left".into(),
            second_pattern: "ooooo xoooo".into(),
            second_device: "right".into(),
            combo_window_ms: 150,
            actual_gap_ms: 290,
        };
        if let DebugEvent::ComboTimeout {
            combo_window_ms,
            actual_gap_ms,
            ..
        } = ev
        {
            assert_eq!(combo_window_ms, 150);
            assert_eq!(actual_gap_ms, 290);
        } else {
            panic!("wrong variant");
        }
    }
}
