use std::time::Instant;

use crate::engine::DeviceId;

/// A single raw tap event received from the BLE layer.
///
/// The BLE layer creates one `RawTapEvent` per notification received from a Tap
/// device and hands it to the engine via [`ComboEngine::push_event`].
///
/// [`ComboEngine::push_event`]: crate::engine::ComboEngine::push_event
#[derive(Debug, Clone)]
pub struct RawTapEvent {
    /// Which device produced this event (e.g. `"left"`, `"right"`, `"solo"`).
    pub device_id: DeviceId,
    /// Raw tap code byte as received from the device. Valid range is `0–31`.
    /// Bit 0 = thumb, bit 4 = pinky (hardware-normalised regardless of hand).
    pub tap_code: u8,
    /// Wall-clock time at which the notification was received.
    /// Set by the BLE layer using `Instant::now()` at the moment of receipt.
    pub received_at: Instant,
}

impl RawTapEvent {
    /// Convenience constructor for building an event with a known timestamp.
    /// Primarily used in tests.
    pub fn new_at(device_id: impl Into<DeviceId>, tap_code: u8, received_at: Instant) -> Self {
        Self {
            device_id: device_id.into(),
            tap_code,
            received_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_tap_event_stores_all_fields() {
        let now = Instant::now();
        let ev = RawTapEvent {
            device_id: DeviceId::new("solo"),
            tap_code: 1,
            received_at: now,
        };
        assert_eq!(ev.device_id.as_str(), "solo");
        assert_eq!(ev.tap_code, 1);
        assert_eq!(ev.received_at, now);
    }

    #[test]
    fn raw_tap_event_new_at_creates_correctly() {
        let now = Instant::now();
        let ev = RawTapEvent::new_at("left", 4, now);
        assert_eq!(ev.device_id.as_str(), "left");
        assert_eq!(ev.tap_code, 4);
        assert_eq!(ev.received_at, now);
    }
}
