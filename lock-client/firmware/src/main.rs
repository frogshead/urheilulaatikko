#![no_std]
#![no_main]

mod actuator;
mod clock;
mod wiegand_capture;

use esp_hal::{
    delay::Delay,
    gpio::{Input, InputConfig, Io, Level, Output, OutputConfig, Pull},
    main,
    time::{Duration, Instant},
};
use esp_println::println;
use lock_client_core::{
    lock::{Actuator, LockController, Outcome},
    wiegand::decode_frame,
};

use crate::{actuator::BoltActuator, clock::SystemClock};

// Required by espflash/the ESP-IDF bootloader to validate the image before
// flashing — without it, flashing is refused with a missing-app-descriptor
// error (found by actually trying to flash a board, not by `cargo build`).
esp_bootloader_esp_idf::esp_app_desc!();

// Registers the panic handler that prints a backtrace over UART.
use esp_backtrace as _;

/// TOTP secret shared with `totp-server`'s provisioning endpoint.
///
/// TODO: this is a compile-time placeholder, same as the very first version
/// of this crate (`b"top secret"`). A real deployment must not ship a secret
/// baked into the firmware image — it needs to be provisioned into
/// non-volatile storage (e.g. ESP32 NVS) separately from the code, so that
/// re-flashing firmware doesn't require re-provisioning every lock, and so
/// the secret isn't sitting in a binary that gets shared for debugging.
/// Not implemented yet.
const SECRET: &[u8] = b"CHANGE ME: provision a real per-lock secret";

/// Steps of 30-second clock drift to tolerate. Conceptually the same
/// parameter as `TOTP_SKEW_STEPS` on the server (see its README), but
/// configured independently here since the lock, not the server, is what
/// actually applies it.
const SKEW_STEPS: u8 = 1;

/// How long the bolt stays released after a correct code.
const UNLOCK_DURATION: Duration = Duration::from_secs(5);

/// How often the main loop polls for a completed Wiegand frame and checks
/// whether it's time to re-lock. This is coarse compared to the
/// microsecond-scale Wiegand pulses themselves, which are timestamped
/// precisely in the interrupt handler regardless of this loop's cadence — it
/// only needs to be short compared to [`wiegand_capture`]'s message-gap
/// timeout and to `UNLOCK_DURATION`.
const POLL_INTERVAL_MS: u32 = 5;

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());

    let mut io = Io::new(peripherals.IO_MUX);
    io.set_interrupt_handler(wiegand_capture::on_gpio_interrupt);

    // Wiegand D0/D1 are open-collector lines that idle high and pulse low
    // briefly for each bit. Pin numbers here are placeholders — wire them to
    // match the actual connection to the LUCKING DOOR S601/S602 keypad, and
    // update these constants to match (see the parts list for the
    // level-shifting note: the keypad's Wiegand lines are commonly 5V logic
    // against this chip's 3.3V GPIO). GPIO2/3 are plain, unrestricted pins on
    // the ESP32-C6-DevKitC-1 (confirmed against both the board's user guide
    // and esp-hal's chip metadata — see the relay pin's comment below for
    // the pins that are NOT safe to pick here).
    let d0 = Input::new(
        peripherals.GPIO2,
        InputConfig::default().with_pull(Pull::Up),
    );
    let d1 = Input::new(
        peripherals.GPIO3,
        InputConfig::default().with_pull(Pull::Up),
    );
    wiegand_capture::install(d0, d1);

    // Drives the level-shifter/relay for the Asia-Teco drop bolt. Starts low
    // (locked); the bolt itself is fail-secure (NC), but the firmware should
    // never rely on that alone to keep the box shut on boot.
    //
    // GPIO10 is a plain, unrestricted pin on this board. GPIO4 (used here on
    // the earlier ESP32-C3 build) is NOT safe on the C6: it's a strapping pin
    // that also carries JTAG (MTMS), so driving it could change how the chip
    // boots. GPIO8/9/15 are strapping-only, GPIO6/7/12/13 carry JTAG/USB
    // debug, and GPIO16/17 are the UART console — avoid those for the same
    // reason if this pin assignment ever needs to change again.
    let relay = Output::new(peripherals.GPIO10, Level::Low, OutputConfig::default());
    let mut actuator = BoltActuator::new(relay);

    let clock = SystemClock::start();
    let mut controller = LockController::new(SECRET, SKEW_STEPS);
    let delay = Delay::new();

    let mut relock_at: Option<Instant> = None;

    println!("lock-client ready");

    loop {
        if let Some(bits) = wiegand_capture::take_completed_frame() {
            let frame = decode_frame(&bits);
            match controller.on_frame(frame, &clock, &mut actuator) {
                Outcome::Unlocked => {
                    // Never log the code or the secret — only the outcome.
                    println!("access granted");
                    relock_at = Some(Instant::now() + UNLOCK_DURATION);
                }
                Outcome::Rejected => println!("access denied"),
                Outcome::Waiting => {}
            }
        }

        if let Some(at) = relock_at {
            if Instant::now() >= at {
                actuator.lock();
                relock_at = None;
                println!("re-locked");
            }
        }

        delay.delay_millis(POLL_INTERVAL_MS);
    }
}
