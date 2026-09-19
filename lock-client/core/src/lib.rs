#![cfg_attr(not(test), no_std)]

//! Hardware-independent decision logic for the lock's TOTP-based keypad.
//!
//! Nothing in this crate touches hardware. The firmware crate (see
//! `../firmware`) captures raw Wiegand pulses in a GPIO interrupt handler,
//! keeps wall-clock time, and drives the physical bolt; it hands the
//! completed frames it decodes to [`lock::LockController`], which is the only
//! thing that knows what a "correct code" means. Splitting it this way means
//! the actual access-control decision can be unit tested on a normal host
//! with `cargo test`, rather than only observed on real hardware.

pub mod lock;
pub mod pin_entry;
pub mod totp;
pub mod wiegand;
