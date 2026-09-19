//! Ties decoded Wiegand key events to TOTP verification and lock actuation.
//!
//! This is the only piece of the firmware that decides what a "correct code"
//! means. It is hardware-independent: the firmware supplies a [`Clock`]
//! (wall-clock Unix seconds) and an [`Actuator`] (drives the physical bolt),
//! and feeds it the [`Frame`]s it decodes from raw Wiegand pulses.

use crate::{
    pin_entry::{Key, PinEntry, Submission},
    totp,
    wiegand::Frame,
};

/// Reads the current wall-clock time as Unix seconds.
///
/// Implemented by the firmware using a monotonic hardware timer plus a
/// boot-time epoch baked in at build time — see the "Known limitation" note
/// on `SystemClock` in `firmware/src/clock.rs`. It has no battery-backed RTC
/// and no network time source, so accuracy depends on the device not losing
/// power between flashing and use.
pub trait Clock {
    fn unix_seconds(&self) -> u64;
}

/// Drives the physical lock. Implementations must make both methods
/// idempotent — calling either while already in that state must be a no-op,
/// not an error — since the controller does not track the bolt's own state.
pub trait Actuator {
    fn unlock(&mut self);
    fn lock(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Still entering digits, or a card/unrecognized frame was ignored.
    Waiting,
    /// A correct code was entered; the actuator has already been unlocked.
    Unlocked,
    /// A full 6-digit code was entered but matched no accepted window.
    Rejected,
}

pub struct LockController<'a> {
    secret: &'a [u8],
    skew_steps: u8,
    entry: PinEntry,
}

impl<'a> LockController<'a> {
    pub const fn new(secret: &'a [u8], skew_steps: u8) -> Self {
        LockController {
            secret,
            skew_steps,
            entry: PinEntry::new(),
        }
    }

    /// Feeds one decoded Wiegand frame.
    ///
    /// Card reads and unrecognized frames are ignored outright rather than
    /// being fed to the PIN buffer — an unrecognized frame must never be
    /// treated as a digit or a wildcard match.
    pub fn on_frame(
        &mut self,
        frame: Frame,
        clock: &impl Clock,
        actuator: &mut impl Actuator,
    ) -> Outcome {
        let key = match frame {
            Frame::Digit(d) => Key::Digit(d),
            Frame::Cancel => Key::Cancel,
            Frame::Enter => Key::Enter,
            Frame::Card { .. } | Frame::Unrecognized { .. } => return Outcome::Waiting,
        };

        match self.entry.push(key) {
            Submission::Pending => Outcome::Waiting,
            Submission::Ready(code) => {
                if totp::matches(self.secret, &code, clock.unix_seconds(), self.skew_steps) {
                    actuator.unlock();
                    Outcome::Unlocked
                } else {
                    Outcome::Rejected
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use totp_embed::{totp_custom, Sha1};

    struct FixedClock(u64);
    impl Clock for FixedClock {
        fn unix_seconds(&self) -> u64 {
            self.0
        }
    }

    #[derive(Default)]
    struct RecordingActuator {
        unlock_calls: u32,
        lock_calls: u32,
    }
    impl Actuator for RecordingActuator {
        fn unlock(&mut self) {
            self.unlock_calls += 1;
        }
        fn lock(&mut self) {
            self.lock_calls += 1;
        }
    }

    /// Digit + Enter frames for a 6-digit code, in entry order.
    fn frames_for(code: u32) -> [Frame; 7] {
        assert!(code < 1_000_000);
        let mut frames = [Frame::Enter; 7];
        let mut remaining = code;
        for slot in frames[..6].iter_mut().rev() {
            *slot = Frame::Digit((remaining % 10) as u8);
            remaining /= 10;
        }
        frames
    }

    #[test]
    fn unlocks_on_a_correct_code() {
        let secret = b"12345678901234567890";
        let code = totp_custom::<Sha1>(30, 6, secret, 59) as u32;
        let mut controller = LockController::new(secret, 0);
        let clock = FixedClock(59);
        let mut actuator = RecordingActuator::default();

        let mut outcome = Outcome::Waiting;
        for frame in frames_for(code) {
            outcome = controller.on_frame(frame, &clock, &mut actuator);
        }

        assert_eq!(outcome, Outcome::Unlocked);
        assert_eq!(actuator.unlock_calls, 1);
        assert_eq!(actuator.lock_calls, 0);
    }

    #[test]
    fn rejects_a_wrong_code_without_unlocking() {
        let secret = b"12345678901234567890";
        let mut controller = LockController::new(secret, 0);
        let clock = FixedClock(59);
        let mut actuator = RecordingActuator::default();

        let mut outcome = Outcome::Waiting;
        for frame in frames_for(0) {
            outcome = controller.on_frame(frame, &clock, &mut actuator);
        }

        assert_eq!(outcome, Outcome::Rejected);
        assert_eq!(actuator.unlock_calls, 0);
    }

    #[test]
    fn ignores_card_reads_and_garbage_frames() {
        let mut controller = LockController::new(b"secret", 0);
        let clock = FixedClock(0);
        let mut actuator = RecordingActuator::default();

        assert_eq!(
            controller.on_frame(
                Frame::Card {
                    facility_code: 1,
                    card_number: 2
                },
                &clock,
                &mut actuator
            ),
            Outcome::Waiting
        );
        assert_eq!(
            controller.on_frame(Frame::Unrecognized { bit_count: 8 }, &clock, &mut actuator),
            Outcome::Waiting
        );
        assert_eq!(actuator.unlock_calls, 0);
    }

    #[test]
    fn cancel_clears_a_partial_entry_before_a_correct_code() {
        let secret = b"12345678901234567890";
        let code = totp_custom::<Sha1>(30, 6, secret, 59) as u32;
        let mut controller = LockController::new(secret, 0);
        let clock = FixedClock(59);
        let mut actuator = RecordingActuator::default();

        controller.on_frame(Frame::Digit(9), &clock, &mut actuator);
        controller.on_frame(Frame::Digit(9), &clock, &mut actuator);
        controller.on_frame(Frame::Cancel, &clock, &mut actuator);

        let mut outcome = Outcome::Waiting;
        for frame in frames_for(code) {
            outcome = controller.on_frame(frame, &clock, &mut actuator);
        }
        assert_eq!(outcome, Outcome::Unlocked);
        assert_eq!(actuator.unlock_calls, 1);
    }

    #[test]
    fn a_wrong_attempt_does_not_poison_the_next_one() {
        let secret = b"12345678901234567890";
        let code = totp_custom::<Sha1>(30, 6, secret, 59) as u32;
        let mut controller = LockController::new(secret, 0);
        let clock = FixedClock(59);
        let mut actuator = RecordingActuator::default();

        for frame in frames_for(0) {
            controller.on_frame(frame, &clock, &mut actuator);
        }
        assert_eq!(actuator.unlock_calls, 0);

        let mut outcome = Outcome::Waiting;
        for frame in frames_for(code) {
            outcome = controller.on_frame(frame, &clock, &mut actuator);
        }
        assert_eq!(outcome, Outcome::Unlocked);
        assert_eq!(actuator.unlock_calls, 1);
    }

    #[test]
    fn honors_configured_skew() {
        let secret = b"12345678901234567890";
        let previous_window_code = totp_custom::<Sha1>(30, 6, secret, 0) as u32; // [0,30)
        let mut controller = LockController::new(secret, 1);
        let clock = FixedClock(59); // [30,60)
        let mut actuator = RecordingActuator::default();

        let mut outcome = Outcome::Waiting;
        for frame in frames_for(previous_window_code) {
            outcome = controller.on_frame(frame, &clock, &mut actuator);
        }
        assert_eq!(outcome, Outcome::Unlocked);
    }
}
