use std::time::Instant;

use crate::engine::DeviceId;
use crate::types::TriggerPattern;

/// The kind of trigger that was resolved, used when walking the layer stack
/// to find a matching [`Trigger`](crate::types::Trigger) variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedTriggerKind {
    /// Matched as a single tap.
    Tap,
    /// Matched as a double-tap (same code tapped twice within the window).
    DoubleTap,
    /// Matched as a triple-tap (same code tapped three times within the window).
    TripleTap,
}

/// A fully-resolved tap trigger, ready to be dispatched against the layer
/// stack.
///
/// The combo/sequence engine produces `ResolvedEvent` values and passes them
/// to the dispatch layer, which walks the [`LayerStack`](crate::engine::LayerStack)
/// looking for a matching mapping.
#[derive(Debug, Clone)]
pub struct ResolvedEvent {
    /// The finger pattern of the resolved event.
    pub pattern: TriggerPattern,
    /// Which device produced the tap(s) that formed this event.
    pub device_id: DeviceId,
    /// Wall-clock time of the first (or only) raw event in this resolution.
    pub received_at: Instant,
    /// Kind of trigger resolution (tap, double-tap, triple-tap).
    pub kind: ResolvedTriggerKind,
    /// How many milliseconds elapsed between `received_at` and the moment the
    /// engine committed to this resolution. Zero for immediate resolutions.
    pub waited_ms: u64,
    /// The timing window (ms) that governed this resolution, for debug display.
    /// Zero for immediate resolutions (no window applies).
    pub window_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TapCode, TriggerPattern};

    fn sample_event(kind: ResolvedTriggerKind) -> ResolvedEvent {
        ResolvedEvent {
            pattern: TriggerPattern::Single(TapCode::from_u8(1).unwrap()),
            device_id: DeviceId::new("solo"),
            received_at: Instant::now(),
            kind,
            waited_ms: 0,
            window_ms: 0,
        }
    }

    #[test]
    fn resolved_event_tap_stores_kind() {
        let ev = sample_event(ResolvedTriggerKind::Tap);
        assert_eq!(ev.kind, ResolvedTriggerKind::Tap);
    }

    #[test]
    fn resolved_event_double_tap_stores_kind() {
        let ev = sample_event(ResolvedTriggerKind::DoubleTap);
        assert_eq!(ev.kind, ResolvedTriggerKind::DoubleTap);
    }

    #[test]
    fn resolved_event_triple_tap_stores_kind() {
        let ev = sample_event(ResolvedTriggerKind::TripleTap);
        assert_eq!(ev.kind, ResolvedTriggerKind::TripleTap);
    }

    #[test]
    fn resolved_event_waited_ms_stored_correctly() {
        let mut ev = sample_event(ResolvedTriggerKind::Tap);
        ev.waited_ms = 247;
        assert_eq!(ev.waited_ms, 247);
    }
}
