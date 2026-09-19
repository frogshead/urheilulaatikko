//! TOTP verification, mirroring the parameters `totp-server` uses (SHA-1, 6
//! digits, 30-second step — see `totp-server/src/totp/mod.rs`) so the same
//! secret produces the same code on both ends.

use totp_embed::{totp_custom, Sha1};

use crate::pin_entry::PIN_LEN;

pub const DIGITS: u32 = 6;
pub const STEP_SECONDS: u64 = 30;

/// Checks an entered code (as decimal digits `0`-`9`, oldest first) against
/// the code for `now` and, if `skew_steps` is nonzero, the windows before and
/// after it.
///
/// Clock-drift tolerance lives here, on the lock, rather than on the server:
/// the server always returns the code for its own current time step, and
/// trusts the lock to accept a code from an adjacent step if its clock has
/// drifted (see `TOTP_SKEW_STEPS` in the server's README).
pub fn matches(secret: &[u8], entered: &[u8; PIN_LEN], now: u64, skew_steps: u8) -> bool {
    let entered_value = digits_to_value(entered);

    for step in -(skew_steps as i64)..=(skew_steps as i64) {
        let candidate_time = now.saturating_add_signed(step * STEP_SECONDS as i64);
        // `totp_custom` returns the numeric value rather than a zero-padded
        // string, so a code with leading zeros (e.g. "007123") must be
        // compared as the number 7123, not string-matched.
        if totp_custom::<Sha1>(STEP_SECONDS, DIGITS, secret, candidate_time) as u32 == entered_value
        {
            return true;
        }
    }
    false
}

fn digits_to_value(digits: &[u8; PIN_LEN]) -> u32 {
    digits.iter().fold(0u32, |acc, &d| acc * 10 + d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digits(code: u32) -> [u8; PIN_LEN] {
        assert!(code < 1_000_000, "code must fit in six digits");
        let mut out = [0u8; PIN_LEN];
        let mut remaining = code;
        for slot in out.iter_mut().rev() {
            *slot = (remaining % 10) as u8;
            remaining /= 10;
        }
        out
    }

    /// RFC 6238 Appendix B, HMAC-SHA1: the ASCII secret "12345678901234567890"
    /// at T=59 yields 94287082 (8 digits); the low 6 digits are 287082 (same
    /// vector `totp-server/src/totp/mod.rs` checks against).
    #[test]
    fn matches_the_rfc_6238_test_vector() {
        let secret = b"12345678901234567890";
        assert!(matches(secret, &digits(287082), 59, 0));
    }

    #[test]
    fn rejects_a_wrong_code() {
        let secret = b"12345678901234567890";
        assert!(!matches(secret, &digits(0), 59, 0));
    }

    #[test]
    fn zero_skew_rejects_the_adjacent_window() {
        let secret = b"12345678901234567890";
        let earlier = totp_custom::<Sha1>(30, 6, secret, 0) as u32; // window [0,30)
        assert!(!matches(secret, &digits(earlier), 59, 0)); // now is in [30,60)
    }

    #[test]
    fn skew_of_one_accepts_the_previous_and_next_window() {
        let secret = b"12345678901234567890";
        // `now` = 59 falls in window index 1, [30,60).
        let earlier = totp_custom::<Sha1>(30, 6, secret, 0) as u32; // window index 0, [0,30)
        let later = totp_custom::<Sha1>(30, 6, secret, 60) as u32; // window index 2, [60,90)

        assert!(matches(secret, &digits(earlier), 59, 1));
        assert!(matches(secret, &digits(later), 59, 1));
    }

    #[test]
    fn skew_of_one_still_rejects_two_windows_away() {
        let secret = b"12345678901234567890";
        // `now` = 59 falls in window index 1; index 3 is two windows away.
        let two_away = totp_custom::<Sha1>(30, 6, secret, 90) as u32; // window index 3, [90,120)
        assert!(!matches(secret, &digits(two_away), 59, 1));
    }

    #[test]
    fn a_leading_zero_code_converts_to_the_same_number_as_the_padded_string() {
        // "007123" must compare equal to 7123, not fail because the string
        // representations differ.
        assert_eq!(digits_to_value(&[0, 0, 7, 1, 2, 3]), 7123);
        assert_eq!(digits_to_value(&digits(7123)), 7123);
    }
}
