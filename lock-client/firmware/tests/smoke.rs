//! Smoke tests that run on real hardware over `probe-rs` (see
//! [embedded-test](https://docs.rs/embedded-test)): `cargo test --release`
//! from `firmware/` flashes this file as its own test binary, resets the
//! board between every test, and reports pass/fail back like `cargo test`
//! normally does on a host — no manual `probe-rs read`/sarja console
//! watching needed.
//!
//! This is deliberately narrow. `core`'s own tests already check the
//! Wiegand/PIN/TOTP/lock logic thoroughly on the host; what they *cannot*
//! check is whether that logic, cross-compiled for `riscv32imac` and
//! actually executed by real silicon, still agrees with itself — and
//! whether the GPIO setup in `main.rs` does what it says. That's all this
//! file checks, nothing about the real Wiegand keypad or relay wiring
//! (see "Known limitations" in ../README.md for what's still unverified).

#![no_std]
#![no_main]

use lock_client_core::{
    lock::{Actuator, Clock, LockController, Outcome},
    wiegand::{decode_frame, Frame},
};

esp_bootloader_esp_idf::esp_app_desc!();

/// A clock that never advances — these tests don't care about clock drift,
/// only about whether a known-good code is accepted at all on this target.
struct FixedClock(u64);

impl Clock for FixedClock {
    fn unix_seconds(&self) -> u64 {
        self.0
    }
}

/// Minimal wrapper so these tests can drive a real GPIO pin through
/// `LockController` without reaching into `main.rs`'s private modules (this
/// package has no library target for integration tests to link against —
/// see "Why this test file is standalone" below).
struct RelayActuator<'d>(esp_hal::gpio::Output<'d>);

impl Actuator for RelayActuator<'_> {
    fn unlock(&mut self) {
        self.0.set_high();
    }

    fn lock(&mut self) {
        self.0.set_low();
    }
}

#[cfg(test)]
#[embedded_test::tests]
mod tests {
    use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};

    use super::*;

    /// Same pins `main.rs` uses — see its comment on the relay pin for which
    /// ESP32-C6-DevKitC-1 pins are strapping/JTAG/USB/UART and must stay
    /// off-limits if these ever need to change.
    struct Hardware {
        relay: RelayActuator<'static>,
    }

    #[init]
    fn init() -> Hardware {
        let peripherals = esp_hal::init(esp_hal::Config::default());

        // Constructed and immediately dropped: this only confirms that
        // configuring the Wiegand input pins (as main.rs does, pull-up
        // inputs) doesn't panic or conflict with the relay pin below —
        // nothing reads them, since no signal generator is wired up here.
        let _d0 = Input::new(
            peripherals.GPIO2,
            InputConfig::default().with_pull(Pull::Up),
        );
        let _d1 = Input::new(
            peripherals.GPIO3,
            InputConfig::default().with_pull(Pull::Up),
        );

        let relay = Output::new(peripherals.GPIO10, Level::Low, OutputConfig::default());

        Hardware {
            relay: RelayActuator(relay),
        }
    }

    /// Confirms the pipeline itself works — built, flashed over JTAG,
    /// executed on real silicon, result reported back — before trusting any
    /// of the more specific tests below. Deliberately asserts nothing: just
    /// returning is the whole point of this one.
    #[test]
    fn hardware_boots_and_reports() {}

    /// The relay must come up low (bolt locked) the instant it's configured,
    /// never defaulting high. This was confirmed once by hand with
    /// `probe-rs read --chip esp32c6 b32 0x60091004 1` (see ../README.md);
    /// this test checks the same fact on every run instead of only once.
    #[test]
    fn relay_starts_locked(hw: Hardware) {
        assert!(hw.relay.0.is_set_low());
    }

    /// `core::wiegand::decode_frame`, cross-compiled for `riscv32imac` and
    /// actually executed here — `core`'s own test for this only runs on the
    /// host (x86_64).
    #[test]
    fn wiegand_decodes_a_digit_on_real_silicon() {
        let five = [false, true, false, true]; // 0b0101
        assert_eq!(decode_frame(&five), Frame::Digit(5));
    }

    /// The RFC 6238 Appendix B test vector (same one `core::totp`'s own host
    /// test checks), run here as a target-specific regression check: it
    /// would catch an `embedded_hal`/`totp_embed` SHA-1 bug that only shows
    /// up on real RISC-V, which the host build and the esp32sim emulator
    /// (which models the CPU, not silicon) can't.
    #[test]
    fn totp_matches_the_rfc_6238_vector_on_real_silicon() {
        let secret = b"12345678901234567890";
        let code_287082 = [2, 8, 7, 0, 8, 2];
        assert!(lock_client_core::totp::matches(secret, &code_287082, 59, 0));
    }

    /// The full Wiegand -> TOTP -> relay decision, against a *real* GPIO
    /// pin this time rather than `core`'s fake `Actuator` or the simulator's
    /// emulated one. This is the smoke test closest to what the lock
    /// actually does, short of a real Wiegand signal on GPIO2/3.
    #[test]
    fn lock_controller_drives_the_real_relay_on_a_correct_code(mut hw: Hardware) {
        let secret = b"12345678901234567890";
        let mut controller = LockController::new(secret, 0);
        let clock = FixedClock(59);

        // 287082, one Digit frame per digit, then Enter.
        let frames = [
            Frame::Digit(2),
            Frame::Digit(8),
            Frame::Digit(7),
            Frame::Digit(0),
            Frame::Digit(8),
            Frame::Digit(2),
            Frame::Enter,
        ];

        let mut outcome = Outcome::Waiting;
        for frame in frames {
            outcome = controller.on_frame(frame, &clock, &mut hw.relay);
        }

        assert_eq!(outcome, Outcome::Unlocked);
        assert!(hw.relay.0.is_set_high());
    }

    /// The mirror image of the test above: a wrong code must never reach
    /// the relay.
    #[test]
    fn lock_controller_never_drives_the_relay_on_a_wrong_code(mut hw: Hardware) {
        let secret = b"12345678901234567890";
        let mut controller = LockController::new(secret, 0);
        let clock = FixedClock(59);

        let frames = [
            Frame::Digit(0),
            Frame::Digit(0),
            Frame::Digit(0),
            Frame::Digit(0),
            Frame::Digit(0),
            Frame::Digit(0),
            Frame::Enter,
        ];

        let mut outcome = Outcome::Waiting;
        for frame in frames {
            outcome = controller.on_frame(frame, &clock, &mut hw.relay);
        }

        assert_eq!(outcome, Outcome::Rejected);
        assert!(hw.relay.0.is_set_low());
    }
}

// Why this test file is standalone (no `use crate::...`):
//
// `lock-client-firmware` has only a `[[bin]]` target (`main.rs`), not a
// `[lib]` — a deliberate choice for the normal firmware, which needs no
// library surface. Cargo integration tests under `tests/` can only `use`
// a package's *library* crate, not its binary's private modules, so this
// file can't reach `main.rs`'s `actuator`/`clock`/`wiegand_capture` modules
// and instead talks to `esp-hal` and `lock-client-core` directly, the same
// way `main.rs` does. `RelayActuator` above is `main.rs`'s `BoltActuator`
// in miniature, kept in sync by hand — if the real one's `unlock`/`lock`
// semantics ever change, update both.
