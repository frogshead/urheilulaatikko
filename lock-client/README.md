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

The workspace has one shared `target/` directory at its root
(`lock-client/target/`, not `lock-client/firmware/target/`), since Cargo
workspaces share one build output directory — the firmware ELF lands at
`lock-client/target/riscv32imc-unknown-none-elf/release/lock-client-firmware`
regardless of which member directory you build from.

Flashing needs [`espflash`](https://github.com/esp-rs/espflash) — install
with `cargo install espflash --locked` (a version compatible with the pinned
toolchain; see the note below). From `firmware/`:

```bash
espflash flash --port /dev/ttyACM0 ../target/riscv32imc-unknown-none-elf/release/lock-client-firmware
espflash monitor --port /dev/ttyACM0    # interactive; needs a real terminal
```

**This has been flashed to a real ESP32-C3-DevKit-RUST-2** (native
USB-JTAG/Serial, shows up as `/dev/ttyACM0` on Linux) and confirmed to boot
past the ESP-IDF second-stage bootloader into the app, print `lock-client
ready`, and then sit idle without panicking or reboot-looping — expected,
since nothing prints again until a Wiegand frame arrives. Two things only
showed up at this step, not from `cargo build`:

- **`esp_bootloader_esp_idf::esp_app_desc!()` is required.** Without it,
  `espflash flash` refuses with a missing-app-descriptor error — the
  bootloader validates this before accepting an image. It's a one-line macro
  call in `main.rs`; `cargo build` doesn't check for it, only the flash tool
  does.
- **`espflash` needs upgrading past whatever the OS/package manager ships.**
  4.3.0 connected to the board fine but failed the actual flash step with an
  opaque `No such file or directory` error; upgrading to `4.5.0` (the newest
  version still compatible with rustc 1.89 — `4.6.0` needs 1.95) fixed it.

**Toolchain note:** the `esp-hal` dependency stack is pinned to exact versions
(`esp-hal =1.1.2`, `esp-backtrace =0.19.0`, `esp-println =0.17.0`,
`esp-bootloader-esp-idf =0.5.0`, plus a downgrade of the transitive
`esp-rom-sys` to 0.1.4) because newer releases require rustc 1.95, ahead of
the 1.89 toolchain this was built and verified against. Bumping these is fine
once the project's toolchain moves past 1.95 — just re-check that
`cargo build --release` (from `firmware/`) still links and flashes.

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
- **Only boot has been verified on real hardware so far** — flashed and
  confirmed it reaches `lock-client ready` and stays stable with nothing
  connected to GPIO2/3/4. The Wiegand capture, decode, and relay-drive path
  has only been exercised by `core`'s unit tests with fake pulses, not against
  a real keypad — that's still open, on top of the wire-format and
  level-shifting unknowns above.
