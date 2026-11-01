# lock-client

Runs on the lock itself: reads a code typed on a Wiegand keypad, checks it
against the same TOTP secret `totp-server` uses, and drives the physical bolt.
The lock never talks to the server, or to anything else — it is offline by
design.

This is a two-crate Cargo workspace:

| Crate | What it is |
| --- | --- |
| [`core/`](core) | Hardware-independent decision logic: Wiegand frame decoding, PIN buffering, TOTP verification, the lock's state machine. No `esp-hal`, no hardware — plain `#![no_std]` (with `std` available under `cfg(test)`), fully covered by `cargo test`. |
| [`firmware/`](firmware) | The actual `no_std` binary for the **ESP32-C6-DevKitC-1**. Wires `core`'s logic to real GPIO interrupts, a monotonic timer, and a relay output. |

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
rustup target add riscv32imac-unknown-none-elf
cd firmware
cargo build --release
```

The workspace has one shared `target/` directory at its root
(`lock-client/target/`, not `lock-client/firmware/target/`), since Cargo
workspaces share one build output directory — the firmware ELF lands at
`lock-client/target/riscv32imac-unknown-none-elf/release/lock-client-firmware`
regardless of which member directory you build from.

### Flashing and debugging

`firmware/.cargo/config.toml` sets `cargo run --release` (from `firmware/`) to
flash and start the firmware via [`probe-rs`](https://probe.rs):

```bash
cargo install probe-rs-tools --locked   # ships `probe-rs`, `cargo-flash`, `cargo-embed`
cd firmware
cargo run --release
```

This uses the ESP32-C6-DevKitC-1's **built-in USB-JTAG interface** — the same
USB port used for the console — so no separate debug probe hardware is
needed. `probe-rs-espressif` (pulled in automatically; confirmed by its
presence in `cargo install`'s build output) uses `espflash`'s own IDF
image-building code internally, so it generates and flashes the bootloader
and partition table itself, the same as `espflash flash` does — there's no
separate merge/image-build step to run first.

**Linux setup, one time:** `probe-rs` talks to the USB-JTAG interface
directly (not through a serial device node), which needs udev rules:

```bash
curl -fsSL https://probe.rs/files/69-probe-rs.rules -o /tmp/69-probe-rs.rules
sudo cp /tmp/69-probe-rs.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
```

The rule grants access via `plugdev` group membership or (on a modern
systemd-logind desktop) automatically to the active seat user. Confirm with
`probe-rs list` — it should show the board without `sudo`.

**Debugging** (breakpoints, stepping, register/memory inspection) goes
through the same JTAG connection:

```bash
probe-rs debug --chip esp32c6 ../target/riscv32imac-unknown-none-elf/release/lock-client-firmware  # built-in CLI debugger
probe-rs gdb --chip esp32c6   # starts a GDB server; connect with e.g. riscv32-esp-elf-gdb
```

**The console (`esp_println` output) is a separate connection either way,**
probe-rs or not: ESP32 RISC-V chips have no RTT hardware (that's a Cortex-M/
CoreSight feature), so `esp_println` writes to the USB-JTAG interface's plain
serial side, not something `probe-rs attach` (RTT-only) can read. Open it
with any serial terminal, same as before:

```bash
espflash monitor --port /dev/ttyACM0    # or picocom, minicom, screen, ...
```

Flashing works over plain serial too, without probe-rs or a JTAG connection
at all — install with `cargo install espflash --locked`:

```bash
espflash flash --port /dev/ttyACM0 ../target/riscv32imac-unknown-none-elf/release/lock-client-firmware
espflash monitor --port /dev/ttyACM0
```

`espflash` is also what `firmware/sim` uses to build the merged flash image
the emulator boots from — that usage is unrelated to real hardware and
unaffected by whichever tool you flash a real board with.

**Not yet flashed to a real ESP32-C6-DevKitC-1 or debugged over probe-rs** —
the switch from the earlier ESP32-C3-DevKit-RUST-2 target happened without
one on hand, and no board was connected when probe-rs was set up either; see
[`sim/`](firmware/sim) for what *has* been verified, on an emulated C6, and
the "Known limitations" section below. The C3 board this ran on previously
did confirm two things worth knowing if `espflash` ever mysteriously refuses
to flash this firmware again, since they're properties of the `esp-hal`/
`espflash` toolchain rather than of either chip specifically:

- **`esp_bootloader_esp_idf::esp_app_desc!()` is required.** Without it,
  `espflash flash` refuses with a missing-app-descriptor error — the
  bootloader validates this before accepting an image. It's a one-line macro
  call in `main.rs`; `cargo build` doesn't check for it, only the flash tool
  does. (`probe-rs`'s IDF loader checks for the same descriptor.)
- **An outdated `espflash` can fail the flash step with an opaque error**
  (`No such file or directory`, no further detail) even after connecting to
  the board successfully. If flashing fails like that, `cargo install
  espflash --locked` to get current, first.

**Toolchain note:** `esp-hal` and its companion crates
(`esp-backtrace`/`esp-println`/`esp-bootloader-esp-idf`) track their latest
releases — no version pins. That wasn't always true: while the toolchain here
was rustc 1.89, several of these needed pinning to older releases because
newer ones required rustc 1.95. If `cargo build` in `firmware/` ever starts
complaining about the installed rustc being too old for a dependency, either
`rustup update` or pin the offending crates back down — check with `cargo
build` first, since which crates are affected shifts as the ecosystem moves.

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

### Simulated integration tests

[`sim/`](firmware/sim) runs the actual firmware binary — real GPIO interrupt
handler, real emulated chip — under
[esp32sim](https://github.com/mikroverk/esp32sim) and drives it with a
scripted Wiegand keypad entry: a correct TOTP code must unlock the relay for
exactly `UNLOCK_DURATION` and relock, a wrong one must never touch it. This is
the one path `core`'s unit tests can't reach (they use fake trait objects, not
a real interrupt handler). CI runs this on every push (`simulate_client_firmware`
in `.github/workflows/rust.yml`); see [`sim/README.md`](firmware/sim/README.md)
for local setup — it needs a one-time build of the emulator, not vendored
into this repo.

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

- **No persistent time source.** The ESP32-C6 has no battery-backed RTC, and
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
  D0/D1, GPIO10 for the relay — chosen as plain, unrestricted pins on the
  ESP32-C6-DevKitC-1; see the comment on the relay pin in `main.rs` for which
  pins are strapping/JTAG/USB/UART and must be avoided if this ever changes).
  Update them to match the actual wiring.
- **Not yet verified on real hardware.** The switch to the ESP32-C6 happened
  without a board on hand — see the flashing section above. What *has* been
  verified is boot on the earlier ESP32-C3 board (same `esp-hal`/`espflash`
  toolchain, different chip) and the full Wiegand/TOTP/relay path in
  [esp32sim](firmware/sim), not against a real keypad. The emulator confirmed
  a correct code unlocks the relay for exactly `UNLOCK_DURATION` and a wrong
  one never touches it — real GPIO interrupts on an emulated chip, not fakes
  — but that's not the same as either a real ESP32-C6 or a real Wiegand
  signal on real wires. The wire-format and level-shifting unknowns above are
  both still open precisely because of that gap.
- **`probe-rs` is configured but unverified against real hardware**, for the
  same reason: no board was connected when it was set up. `probe-rs list`
  has not been run against a real ESP32-C6, the udev rules have not been
  installed on this machine (install step above, skipped for now), and
  `cargo run --release` has never actually flashed anything over JTAG. If it
  doesn't connect, falling back to the plain `espflash flash`/`espflash
  monitor` commands above rules out whether the problem is probe-rs/JTAG
  setup specifically.
