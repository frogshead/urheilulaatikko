//! Accumulates keypad digit presses into a fixed-length PIN buffer.

/// Length of a TOTP code — see [`crate::totp`].
pub const PIN_LEN: usize = 6;

/// A keypress, already stripped of its Wiegand encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Digit(u8),
    Cancel,
    Enter,
}

/// What feeding a key into [`PinEntry`] produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Submission {
    /// Still entering digits, or a cancel/reset just happened — nothing to
    /// check yet.
    Pending,
    /// `#` was pressed with exactly [`PIN_LEN`] digits buffered.
    Ready([u8; PIN_LEN]),
}

/// Buffers keystrokes between `Enter`/`Cancel` presses.
#[derive(Debug, Default)]
pub struct PinEntry {
    buffer: [u8; PIN_LEN],
    len: usize,
}

impl PinEntry {
    pub const fn new() -> Self {
        PinEntry {
            buffer: [0; PIN_LEN],
            len: 0,
        }
    }

    /// Feeds one keypress. The buffer is always cleared after `Enter`,
    /// whether or not the caller goes on to accept the code — a rejected
    /// guess must never leave a stale PIN sitting around for the next digit
    /// to append to.
    pub fn push(&mut self, key: Key) -> Submission {
        match key {
            Key::Cancel => {
                self.reset();
                Submission::Pending
            }
            Key::Digit(d) => {
                if self.len < PIN_LEN {
                    self.buffer[self.len] = d;
                    self.len += 1;
                } else {
                    // A 7th digit before Enter almost certainly means the
                    // operator mistyped and kept going. Start a fresh attempt
                    // with this digit rather than silently dropping it.
                    self.buffer[0] = d;
                    self.len = 1;
                }
                Submission::Pending
            }
            Key::Enter => {
                let result = if self.len == PIN_LEN {
                    Submission::Ready(self.buffer)
                } else {
                    Submission::Pending
                };
                self.reset();
                result
            }
        }
    }

    fn reset(&mut self) {
        self.buffer = [0; PIN_LEN];
        self.len = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enter_digits(entry: &mut PinEntry, digits: &[u8]) -> Submission {
        let mut last = Submission::Pending;
        for &d in digits {
            last = entry.push(Key::Digit(d));
        }
        last
    }

    #[test]
    fn six_digits_then_enter_submits_the_code() {
        let mut entry = PinEntry::new();
        enter_digits(&mut entry, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(
            entry.push(Key::Enter),
            Submission::Ready([1, 2, 3, 4, 5, 6])
        );
    }

    #[test]
    fn enter_with_too_few_digits_is_ignored_and_clears_the_buffer() {
        let mut entry = PinEntry::new();
        enter_digits(&mut entry, &[1, 2, 3]);
        assert_eq!(entry.push(Key::Enter), Submission::Pending);

        // The short attempt must not carry over into the next one.
        let result = enter_digits(&mut entry, &[9, 9, 9, 9, 9, 9]);
        assert_eq!(result, Submission::Pending);
        assert_eq!(
            entry.push(Key::Enter),
            Submission::Ready([9, 9, 9, 9, 9, 9])
        );
    }

    #[test]
    fn cancel_clears_a_partial_entry() {
        let mut entry = PinEntry::new();
        enter_digits(&mut entry, &[1, 2, 3]);
        entry.push(Key::Cancel);
        enter_digits(&mut entry, &[4, 5, 6, 7, 8, 9]);
        assert_eq!(
            entry.push(Key::Enter),
            Submission::Ready([4, 5, 6, 7, 8, 9])
        );
    }

    #[test]
    fn a_seventh_digit_restarts_the_attempt_instead_of_being_dropped() {
        let mut entry = PinEntry::new();
        // Six digits, then a 7th before Enter: the 7th becomes the first
        // digit of a fresh attempt instead of being dropped or overflowing
        // the buffer.
        enter_digits(&mut entry, &[1, 2, 3, 4, 5, 6, 7]);
        enter_digits(&mut entry, &[2, 3, 4, 5, 6]);
        assert_eq!(
            entry.push(Key::Enter),
            Submission::Ready([7, 2, 3, 4, 5, 6])
        );
    }

    #[test]
    fn a_submitted_code_does_not_leak_into_the_next_attempt() {
        let mut entry = PinEntry::new();
        enter_digits(&mut entry, &[1, 1, 1, 1, 1, 1]);
        entry.push(Key::Enter);

        enter_digits(&mut entry, &[2, 2, 2]);
        assert_eq!(entry.push(Key::Enter), Submission::Pending);
    }
}
