/// A parsed Tap device data packet.
///
/// Packet format (source: `docs/reference/api-doc.txt`):
///
/// | Byte(s) | Field        | Type  | Notes                              |
/// | ------- | ------------ | ----- | ---------------------------------- |
/// | 0       | `tap_code`   | `u8`  | Bitmask; bit 0 = thumb, bit 4 = pinky |
/// | 1–2     | `interval_ms`| `u16` | ms since last tap, little-endian; saturated at 65535 |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapPacket {
    /// Raw tap bitmask. Bit 0 = thumb (LSB) … bit 4 = pinky. Bits 5–7 unused.
    pub tap_code: u8,
    /// Milliseconds elapsed since the previous tap. Saturated at 65535.
    pub interval_ms: u16,
}

/// Parse a Tap notification payload into a [`TapPacket`].
///
/// Returns `None` for empty slices. Packets shorter than 3 bytes are accepted
/// with `interval_ms` set to `0` (the interval field is informational only).
/// Extra bytes beyond the first 3 are ignored.
pub fn parse_tap_packet(data: &[u8]) -> Option<TapPacket> {
    let tap_code = *data.first()?;
    let interval_ms = if data.len() >= 3 {
        u16::from_le_bytes([data[1], data[2]])
    } else {
        0
    };
    Some(TapPacket {
        tap_code,
        interval_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // ── Empty / undersized ────────────────────────────────────────────────────

    #[test]
    fn parse_tap_packet_empty_slice_returns_none() {
        assert!(parse_tap_packet(&[]).is_none());
    }

    #[test]
    fn parse_tap_packet_one_byte_interval_zero() {
        let p = parse_tap_packet(&[0x05]).unwrap();
        assert_eq!(p.tap_code, 0x05);
        assert_eq!(p.interval_ms, 0);
    }

    #[test]
    fn parse_tap_packet_two_bytes_interval_zero() {
        let p = parse_tap_packet(&[0x05, 0xAB]).unwrap();
        assert_eq!(p.tap_code, 0x05);
        assert_eq!(p.interval_ms, 0);
    }

    // ── All 32 valid tap codes ────────────────────────────────────────────────

    #[rstest]
    fn parse_tap_packet_all_valid_tap_codes_round_trip(
        #[values(
            0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31
        )]
        code: u8,
    ) {
        let p = parse_tap_packet(&[code, 0x00, 0x00]).unwrap();
        assert_eq!(p.tap_code, code);
    }

    // ── Interval encoding ─────────────────────────────────────────────────────

    #[test]
    fn parse_tap_packet_interval_little_endian_1000ms() {
        // 1000 = 0x03E8 → little-endian bytes [0xE8, 0x03]
        let p = parse_tap_packet(&[0x01, 0xE8, 0x03]).unwrap();
        assert_eq!(p.interval_ms, 1000);
    }

    #[test]
    fn parse_tap_packet_interval_zero() {
        let p = parse_tap_packet(&[0x01, 0x00, 0x00]).unwrap();
        assert_eq!(p.interval_ms, 0);
    }

    #[test]
    fn parse_tap_packet_interval_saturated_at_65535() {
        let p = parse_tap_packet(&[0x01, 0xFF, 0xFF]).unwrap();
        assert_eq!(p.interval_ms, 65535);
    }

    // ── Extra bytes ───────────────────────────────────────────────────────────

    #[test]
    fn parse_tap_packet_extra_bytes_beyond_3_are_ignored() {
        let p = parse_tap_packet(&[0x01, 0x00, 0x00, 0xDE, 0xAD]).unwrap();
        assert_eq!(p.tap_code, 0x01);
        assert_eq!(p.interval_ms, 0);
    }

    // ── Specific finger combinations ──────────────────────────────────────────

    #[test]
    fn parse_tap_packet_thumb_only_tap_code_1() {
        let p = parse_tap_packet(&[0x01, 0x00, 0x00]).unwrap();
        assert_eq!(p.tap_code, 1);
    }

    #[test]
    fn parse_tap_packet_all_fingers_tap_code_31() {
        let p = parse_tap_packet(&[0x1F, 0x00, 0x00]).unwrap();
        assert_eq!(p.tap_code, 31);
    }

    #[test]
    fn parse_tap_packet_thumb_and_index_tap_code_3() {
        // Example from api-doc.txt: "thumb + index (character N) = 0x03"
        let p = parse_tap_packet(&[0x03, 0x00, 0x00]).unwrap();
        assert_eq!(p.tap_code, 3);
    }
}
