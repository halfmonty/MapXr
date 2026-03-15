use serde::{Deserialize, Serialize};

/// Which hand wears the device.
///
/// `Hand` determines the read direction of single-hand finger pattern strings:
/// - `Right` (default): thumb-first — `"xoooo"` = thumb tapped.
/// - `Left`: pinky-first — `"xoooo"` = pinky tapped.
///
/// The underlying [`TapCode`](crate::types::TapCode) bit layout is always the
/// same regardless of hand: bit 0 = thumb, bit 4 = pinky. `Hand` only affects
/// how the `"oooox"` string notation is parsed and serialised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Hand {
    /// Thumb is at position 0 (leftmost character) of the pattern string.
    #[default]
    Right,
    /// Pinky is at position 0 (leftmost character) of the pattern string.
    Left,
}
