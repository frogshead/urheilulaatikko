//! Decoding of a captured Wiegand bit frame.
//!
//! The firmware accumulates raw D0/D1 pulses (D0 = a `0` bit, D1 = a `1` bit)
//! into a buffer in its interrupt handler, and hands this module a completed
//! bit sequence once a message-gap timeout has elapsed. This module never
//! touches timing or hardware — it only turns a finished bit sequence into a
//! meaning.

/// A decoded Wiegand frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// A single keypress, decoded from a 4-bit burst: `0`-`9` for digits.
    Digit(u8),
    /// The `*` key: cancels whatever has been entered so far.
    Cancel,
    /// The `#` key: submits the entered code.
    Enter,
    /// A standard 26-bit Wiegand card read. Not used for code entry — the
    /// [`crate::lock::LockController`] ignores these outright. Reported here
    /// only so a caller could log or extend behavior later.
    Card { facility_code: u8, card_number: u16 },
    /// A bit count that matched no format this project understands. Treated
    /// as noise, never as a match — an unrecognized frame must never be
    /// mistaken for a wildcard.
    Unrecognized { bit_count: usize },
}

/// Decodes a captured bit sequence (oldest bit first; `true` = a `1` bit) into
/// a [`Frame`].
///
/// ## Protocol assumptions
///
/// This targets the wire format common to inexpensive "Wiegand 26" access
/// keypads: each keypress is its own 4-bit burst, MSB first (`0000`-`1001`
/// for digits `0`-`9`, `1010` for `*`, `1011` for `#`), and a 26-bit frame is
/// a standard Wiegand card read (1 even-parity bit + 8 facility-code bits +
/// 16 card-number bits + 1 odd-parity bit; parity is not currently checked).
///
/// This has **not been verified against the LUCKING DOOR S601/S602's actual
/// datasheet** — a real unit was not available to capture a reference frame
/// from at the time this was written. If the real keypad uses a different
/// per-key encoding, this is the only function that needs to change; nothing
/// else in this crate depends on the wire format.
pub fn decode_frame(bits: &[bool]) -> Frame {
    match bits.len() {
        4 => decode_nibble(bits),
        26 => decode_card(bits),
        n => Frame::Unrecognized { bit_count: n },
    }
}

fn decode_nibble(bits: &[bool]) -> Frame {
    match bits_to_u32(bits) {
        value @ 0..=9 => Frame::Digit(value as u8),
        10 => Frame::Cancel,
        11 => Frame::Enter,
        _ => Frame::Unrecognized { bit_count: 4 },
    }
}

fn decode_card(bits: &[bool]) -> Frame {
    Frame::Card {
        facility_code: bits_to_u32(&bits[1..9]) as u8,
        card_number: bits_to_u32(&bits[9..25]) as u16,
    }
}

fn bits_to_u32(bits: &[bool]) -> u32 {
    bits.iter()
        .fold(0u32, |acc, &bit| (acc << 1) | (bit as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nibble(value: u8) -> [bool; 4] {
        [
            value & 0b1000 != 0,
            value & 0b0100 != 0,
            value & 0b0010 != 0,
            value & 0b0001 != 0,
        ]
    }

    fn bits_msb_first(value: u32, width: usize) -> std::vec::Vec<bool> {
        (0..width).rev().map(|i| (value >> i) & 1 == 1).collect()
    }

    #[test]
    fn decodes_all_ten_digits() {
        for d in 0..=9u8 {
            assert_eq!(decode_frame(&nibble(d)), Frame::Digit(d));
        }
    }

    #[test]
    fn decodes_star_as_cancel_and_hash_as_enter() {
        assert_eq!(decode_frame(&nibble(10)), Frame::Cancel);
        assert_eq!(decode_frame(&nibble(11)), Frame::Enter);
    }

    #[test]
    fn rejects_impossible_nibble_values_instead_of_guessing() {
        assert_eq!(
            decode_frame(&nibble(12)),
            Frame::Unrecognized { bit_count: 4 }
        );
        assert_eq!(
            decode_frame(&nibble(15)),
            Frame::Unrecognized { bit_count: 4 }
        );
    }

    #[test]
    fn decodes_a_26_bit_card_frame() {
        let mut bits = [false; 26];
        bits[1..9].copy_from_slice(&bits_msb_first(12, 8));
        bits[9..25].copy_from_slice(&bits_msb_first(3000, 16));

        match decode_frame(&bits) {
            Frame::Card {
                facility_code,
                card_number,
            } => {
                assert_eq!(facility_code, 12);
                assert_eq!(card_number, 3000);
            }
            other => panic!("expected Card, got {other:?}"),
        }
    }

    #[test]
    fn reports_unrecognized_lengths_instead_of_guessing() {
        assert_eq!(
            decode_frame(&[true; 8]),
            Frame::Unrecognized { bit_count: 8 }
        );
        assert_eq!(decode_frame(&[]), Frame::Unrecognized { bit_count: 0 });
    }
}
