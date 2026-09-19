# lock-client

Runs on the lock itself: reads a code typed on a Wiegand keypad, checks it
against the same TOTP secret `totp-server` uses, and drives the physical bolt.
The lock never talks to the server, or to anything else — it is offline by
design.

This is a two-crate Cargo workspace:

| Crate | What it is |
| --- | --- |
| [`core/`](core) | Hardware-independent decision logic: Wiegand frame decoding, PIN buffering, TOTP verification, the lock's state machine. No `esp-hal`, no hardware — plain `#![no_std]` (with `std` available under `cfg(test)`), fully covered by `cargo test`. |
| [`firmware/`](firmware) | The actual `no_std` binary for the **ESP32-C3-DevKit-RUST-2**. Wires `core`'s logic to real GPIO interrupts, a monotonic timer, and a relay output. |

Splitting it this way means the part that actually decides "is this code
correct" is tested the same way `totp-server`'s equivalent logic is — with
`cargo test`, on a normal machine — rather than only by flashing real hardware
and trying codes by hand.

## Building

```bash
# core: ordinary host crate, this is what `cargo test` runs against
cargo build --manifest-path ./Cargo.toml     # builds `core` (the workspace default member)
cargo test  --manifest-path ./Cargo.toml

# firmware: must be run from within firmware/, not via --manifest-path — Cargo
# resolves .cargo/config.toml (which sets the target and a required linker
# flag) relative to the working directory, not the manifest path.
rustup target add riscv32imc-unknown-none-elf
cd firmware
cargo build --release
```

The firmware ELF lands at
`target/riscv32imc-unknown-none-elf/release/lock-client-firmware`. Flashing it
needs [`espflash`](https://github.com/esp-rs/espflash)
(`cargo install espflash`); `firmware/.cargo/config.toml` already points
`cargo run` at it (`espflash flash --monitor`).

**Toolchain note:** the `esp-hal` dependency stack is pinned to exact versions
(`esp-hal =1.1.2`, `esp-backtrace =0.19.0`, `esp-println =0.17.0`, plus a
downgrade of the transitive `esp-rom-sys` to 0.1.4) because newer releases
require rustc 1.95, ahead of the 1.89 toolchain this was built and verified
against. Bumping these is fine once the project's toolchain moves past 1.95 —
just re-check that `cargo build --release` (from `firmware/`) still links.

## How it fits together

```
Wiegand D0/D1 pulses (GPIO interrupt, µs-scale timing)
        │
        ▼
firmware::wiegand_capture   — accumulates bits, detects message-gap timeout
        │  (a completed bit sequence)
        ▼
core::wiegand::decode_frame — turns bits into Digit/Cancel/Enter/Card/Unrecognized
        │
        ▼
core::lock::LockController  — buffers digits (core::pin_entry), checks the
        │                       completed code against core::totp
        ▼
firmware::actuator::BoltActuator — drives the GPIO feeding the relay
```

`LockController` never touches hardware directly: it takes a `Clock` (wall
clock, Unix seconds) and an `Actuator` (unlock/lock) as trait objects, which
is what makes it testable with fakes in `core`'s test suite instead of only
observable by flashing a real board.

### TOTP parameters

Same as `totp-server` (see `totp-server/src/totp/mod.rs`): SHA-1, 6 digits,
30-second step. `SKEW_STEPS` in `firmware/src/main.rs` (default 1) is the
lock's side of clock-drift tolerance — the server always returns the code for
its own current time step, and trusts the lock to also accept a code from an
adjacent step.

### Wiegand protocol assumptions — unverified

`core::wiegand::decode_frame` assumes the wire format common to inexpensive
"Wiegand 26" keypads: each keypress is its own 4-bit burst (`0`-`9`, `1010` for
`*`, `1011` for `#`), and a 26-bit frame is a standard card read (ignored — see
below). **This has not been checked against the LUCKING DOOR S601/S602's
actual datasheet**; no unit was available to capture a reference frame from
when this was written. If the real keypad encodes keys differently, only
`decode_frame` needs to change.

Card reads and any other bit length are ignored outright by `LockController` —
they are never treated as a digit or as a wildcard match.

## Known limitations (first pass — see the parts list for more)

- **No persistent time source.** The ESP32-C3 has no battery-backed RTC, and
  this project deliberately has no network time sync (the lock must work with
  no internet connection at all). `firmware::clock::SystemClock` reconstructs
  "now" as *build time + time since this boot* — a power loss resets the
  clock to when the firmware was compiled, not flashed or booted. A real
  deployment needs either a battery-backed RTC module or a provisioning step
  that sets the correct time on every boot.
- **The secret is a compile-time placeholder.** `SECRET` in
  `firmware/src/main.rs` is a hardcoded byte string, the same kind of stub the
  very first version of this crate had. It needs to move to non-volatile
  storage (e.g. ESP32 NVS), provisioned separately from the firmware image —
  not implemented yet.
- **No level shifting designed yet.** The keypad's Wiegand lines are commonly
  5V logic and the relay side is 12V, against this chip's 3.3V GPIO — see the
  parts list (`plywood-box/laatikko-ohje-ja-ostoslista.md`) for the open note.
- **Pin numbers in `firmware/src/main.rs` are placeholders** (GPIO2/3 for
  D0/D1, GPIO4 for the relay). Update them to match the actual wiring.
