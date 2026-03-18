/**
 * Utilities for decoding raw tap_code bytes from the Tap BLE device.
 *
 * Bit layout (fixed by hardware, regardless of which hand wears the device):
 *   bit 0 = thumb, bit 1 = index, bit 2 = middle, bit 3 = ring, bit 4 = pinky
 *
 * The `hand` parameter controls the string encoding direction only:
 *   - "right": position 0 in the string = bit 0 (thumb-first)
 *   - "left":  position 0 in the string = bit 4 (pinky-first)
 *
 * This matches the Rust `TapCode::to_single_pattern(hand)` implementation exactly.
 */

/**
 * Decodes a raw tap_code byte into a 5-character finger-pattern string.
 *
 * @param tapCode Raw u8 byte (0–31) from the BLE notification.
 * @param hand    The hand orientation: "right" (thumb-first) or "left" (pinky-first).
 * @returns A 5-character finger pattern string, e.g. "xoooo".
 */
export function tapCodeToPattern(tapCode: number, hand: "left" | "right"): string {
  let result = "";
  for (let i = 0; i < 5; i++) {
    const bit = hand === "right" ? i : 4 - i;
    result += (tapCode >> bit) & 1 ? "x" : "o";
  }
  return result;
}
