//! Captures raw Wiegand D0/D1 pulses in a GPIO interrupt handler and hands
//! completed frames to the main loop once a message-gap timeout elapses.
//!
//! This is the one piece of Wiegand handling that genuinely needs to run in
//! an interrupt: pulses are on the order of tens of microseconds, far shorter
//! than anything a polling main loop could reliably catch. Everything *after*
//! "here is a completed bit sequence" — what it means, whether it's a valid
//! code — lives in `lock_client_core` and has no idea an interrupt was ever
//! involved.

use core::cell::RefCell;

use critical_section::Mutex;
use esp_hal::{
    gpio::{Event, Input},
    time::{Duration, Instant},
};

/// Longest frame this project understands (a 26-bit card read). Sized with a
/// little headroom; a frame that overruns this just means the oldest bit is
/// dropped, which quietly fails the eventual TOTP comparison rather than
/// panicking.
const MAX_BITS: usize = 34;

/// Silence on both lines for this long means a frame is finished. Cheap
/// Wiegand keypads typically gap 20-100ms between bits within one keypress
/// burst and far longer between separate keypresses; 25ms is a common choice
/// in Wiegand libraries and is treated as a starting point to tune against
/// the real keypad, not a verified spec value.
const MESSAGE_GAP: Duration = Duration::from_millis(25);

struct CaptureState {
    d0: Input<'static>,
    d1: Input<'static>,
    bits: heapless::Vec<bool, MAX_BITS>,
    last_edge: Option<Instant>,
}

static CAPTURE: Mutex<RefCell<Option<CaptureState>>> = Mutex::new(RefCell::new(None));

/// Registers `d0`/`d1` as the Wiegand data lines and starts listening for
/// falling edges. Must be called after the GPIO interrupt handler has been
/// installed via `Io::set_interrupt_handler`.
pub fn install(mut d0: Input<'static>, mut d1: Input<'static>) {
    d0.listen(Event::FallingEdge);
    d1.listen(Event::FallingEdge);

    critical_section::with(|cs| {
        CAPTURE.borrow_ref_mut(cs).replace(CaptureState {
            d0,
            d1,
            bits: heapless::Vec::new(),
            last_edge: None,
        });
    });
}

/// The GPIO interrupt handler. All GPIO pins on the chip share one interrupt
/// vector, so this must check both lines every time it runs.
#[esp_hal::handler]
pub(crate) fn on_gpio_interrupt() {
    critical_section::with(|cs| {
        let mut state = CAPTURE.borrow_ref_mut(cs);
        let Some(state) = state.as_mut() else {
            // The interrupt fired before `install` ran; nothing to do.
            return;
        };

        if state.d0.is_interrupt_set() {
            state.d0.clear_interrupt();
            push_bit(state, false);
        }
        if state.d1.is_interrupt_set() {
            state.d1.clear_interrupt();
            push_bit(state, true);
        }
    });
}

fn push_bit(state: &mut CaptureState, bit: bool) {
    if state.bits.is_full() {
        state.bits.remove(0);
    }
    // `heapless::Vec::push` only fails when full, which was just ruled out.
    let _ = state.bits.push(bit);
    state.last_edge = Some(Instant::now());
}

/// If a frame has been silent for at least [`MESSAGE_GAP`], drains and
/// returns its bits (oldest first). Call this from the main loop; it never
/// blocks.
pub fn take_completed_frame() -> Option<heapless::Vec<bool, MAX_BITS>> {
    critical_section::with(|cs| {
        let mut state = CAPTURE.borrow_ref_mut(cs);
        let state = state.as_mut()?;

        let last_edge = state.last_edge?;
        if state.bits.is_empty() {
            return None;
        }
        if Instant::now() - last_edge < MESSAGE_GAP {
            return None;
        }

        state.last_edge = None;
        Some(core::mem::take(&mut state.bits))
    })
}
