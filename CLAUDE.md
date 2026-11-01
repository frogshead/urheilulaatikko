# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Two top-level projects, each with its own `Cargo.toml`/workspace and built
with `--manifest-path` (not one shared workspace):

- **lock-client**: runs on the offline lock. A two-crate Cargo workspace
  (`lock-client/Cargo.toml`, `default-members = ["core"]`):
  - `lock-client/core`: hardware-independent logic (Wiegand frame decoding,
    PIN buffering, TOTP verification, the lock's state machine). Plain
    `#![no_std]` with `std` available under `cfg(test)`; this is what
    `cargo test` exercises, no hardware or embedded toolchain needed.
  - `lock-client/firmware`: the actual `no_std` binary for the
    **ESP32-C6-DevKitC-1** (RISC-V RV32IMAC,
    `riscv32imac-unknown-none-elf`), built on `esp-hal`. Wires `core`'s logic
    to real GPIO interrupts, a monotonic timer, and a relay output. (An
    earlier version of this crate targeted an ESP32-C3-DevKit-RUST-2; see
    `lock-client/README.md` if you find stray references to that board.)
  - See `lock-client/README.md` for the architecture diagram and known
    limitations (no persistent time source, no NVS-provisioned secret yet,
    unverified Wiegand wire-format assumptions, no level-shifting design).
- **totp-server**: an axum service that issues one-time PINs to users who have
  authenticated against Keycloak. See `totp-server/README.md` for the API,
  configuration, and development environment.

The two ends never talk to each other. The lock is offline; the user carries the
PIN across by typing it in. The only thing they share is the TOTP secret.

**Algorithms are aligned:** both sides use SHA-1, 6 digits, 30-second steps
(RFC 6238 / Google Authenticator profile — see `lock-client/core/src/totp.rs`
and `totp-server/src/totp/mod.rs`), so the same secret produces the same PIN
on both ends.

**Toolchain note:** `lock-client/firmware`'s `esp-hal`/`esp-backtrace`/
`esp-println`/`esp-bootloader-esp-idf` dependencies track latest, unpinned.
They needed exact-version pins for a while when the toolchain here was rustc
1.89 (several newer releases require 1.95) — if `cargo build` in
`lock-client/firmware` ever complains the installed rustc is too old for a
dependency, either `rustup update` or pin the offending crate(s) back down;
see `lock-client/README.md` for the full story. Separately: firmware must be
built by `cd`-ing into `lock-client/firmware` and running plain `cargo
build`/`clippy`/`fmt` there — Cargo resolves `.cargo/config.toml` (which sets
the RISC-V target and a linker flag `esp-hal`'s generated linker script
needs) relative to the *working directory*, not `--manifest-path`, so
invoking it from elsewhere silently skips that config and fails to build or
link.

**Not yet flashed to a real ESP32-C6** — the switch away from the
ESP32-C3-DevKit-RUST-2 happened without a C6 board on hand. The C3 board did
confirm boot (`/dev/ttyACM0` on Linux), both with the original
rustc-1.89-pinned dependency set and again after bumping everything to latest
once the toolchain moved to 1.98, and surfaced two things worth knowing if
`espflash` ever mysteriously refuses to flash this firmware again — see
`lock-client/README.md`: it needs `esp_bootloader_esp_idf::esp_app_desc!()`
in `main.rs` or `espflash` refuses to flash it, and an outdated `espflash`
can fail the flash step with an opaque error even after connecting to the
board fine. These are properties of the `esp-hal`/`espflash` toolchain, not
of either chip specifically, so they still apply.

**`cargo run --release` in `lock-client/firmware` now flashes via `probe-rs`**
(`runner` in `.cargo/config.toml`), over the C6-DevKitC-1's built-in USB-JTAG
— no separate debug probe. `probe-rs-espressif` (installed automatically
with `probe-rs-tools`) uses `espflash`'s own IDF image-building code
internally, so it still generates the bootloader/partition table itself;
`espflash flash`/`espflash monitor` remain documented as a fallback that
needs no JTAG setup. **This is unverified** — no board was connected when it
was configured, so neither the udev rules nor an actual `probe-rs run`/
`probe-rs gdb`/`probe-rs debug` session have been exercised against real
silicon. One thing worth knowing either way: ESP32 RISC-V chips have no RTT
hardware, so `esp_println`'s console output is not visible through
`probe-rs attach` (which is RTT-only) — it still needs a separate serial
connection (`espflash monitor` or any terminal), regardless of which tool
flashed the chip. See `lock-client/README.md`, "Flashing and debugging".

**The Wiegand/TOTP/relay path is verified in `lock-client/firmware/sim`**
against [esp32sim](https://github.com/mikroverk/esp32sim), an
instruction-level ESP32 emulator (a separate project, not vendored here; run
in CI by the `simulate_client_firmware` job at a pinned commit — see
`lock-client/firmware/sim/README.md`). It boots the real firmware binary
under the real mask ROM and bootloader as a bare module (`--board none`),
scripts a Wiegand keypad entry on GPIO2/3, and checks the relay pin (GPIO10)
over a VCD waveform: a correct TOTP code must unlock it for exactly
`UNLOCK_DURATION` and relock, a wrong one must never touch it. This is real
GPIO interrupts on an emulated chip, not the fake `Clock`/`Actuator` trait
objects `core`'s unit tests use — but it is still not a real ESP32-C6 or a
real Wiegand signal on real wires. (Porting this from the C3 to the C6 found
a real esp32sim quirk worth knowing: a GPIO pin's `$var` only appears in a
VCD once that pin actually changes level, so "absent from the VCD" and
"present with zero transitions" have to be treated as the same thing — see
`run_sim_tests.py`'s `parse_vcd_relay_transitions`.)

## Build and Test Commands

```bash
# Build (lock-client: builds `core`, the workspace default member — host-only)
cargo build --manifest-path ./lock-client/Cargo.toml
cargo build --manifest-path ./totp-server/Cargo.toml

# Test (lock-client/core: pure logic; server: unit + wiremock introspection
# tests. Neither needs Docker or an embedded toolchain)
cargo test --manifest-path ./lock-client/Cargo.toml
cargo test --manifest-path ./totp-server/Cargo.toml

# ESP32-C6 firmware — must run from within the directory, see the toolchain
# note above
( cd lock-client/firmware && rustup target add riscv32imac-unknown-none-elf && cargo build --release )

# Wiegand/TOTP/relay integration test against the esp32sim emulator. CI runs
# this too (simulate_client_firmware); locally it needs a one-time build of
# esp32sim first — see sim/README.md.
( cd lock-client/firmware && ESP32SIM_BIN=... ESP32SIM_ROM_DIR=... python3 sim/run_sim_tests.py )

# Server integration tests against real Postgres and Keycloak containers.
# These are #[ignore]d by default and need a working Docker daemon.
cargo test --manifest-path ./totp-server/Cargo.toml --test e2e_integration -- --ignored

# Lints that CI enforces
cargo fmt --manifest-path ./totp-server/Cargo.toml --check
cargo clippy --manifest-path ./totp-server/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path ./lock-client/core/Cargo.toml --check
cargo clippy --manifest-path ./lock-client/core/Cargo.toml --all-targets -- -D warnings
( cd lock-client/firmware && cargo fmt --check && cargo clippy --release -- -D warnings )
```

### Development environment

```bash
cd totp-server
cp .env.example .env
echo "TOTP_ENCRYPTION_KEY=base64:$(openssl rand -base64 32)" >> .env
docker compose up -d --build   # postgres + keycloak + totp-server
./scripts/e2e.sh               # full flow: login -> provision -> PIN
```

If a default host port is taken, override `POSTGRES_PORT`, `KEYCLOAK_PORT`, or
`TOTP_SERVER_PORT` in `.env`.

## Architecture Notes

### lock-client

`firmware::wiegand_capture` timestamps raw D0/D1 pulses in a GPIO interrupt
handler (edges are microsecond-scale; a polling loop could not catch them
reliably) and hands a completed bit sequence to `core::wiegand::decode_frame`
once a message-gap timeout elapses. From there `core::lock::LockController`
buffers digits (`core::pin_entry`) and checks a completed code against
`core::totp` — none of this middle layer touches hardware, which is what
makes it unit-testable. `firmware::actuator::BoltActuator` is the only thing
that actually drives a GPIO pin.

### Server

Request flow: bearer token → `auth::middleware` → RFC 7662 introspection against
Keycloak → `AuthenticatedUser` in the request extensions → rate limiter keyed on
that subject → handler → `SecretStore` (decrypts) → `totp::generate`.

Invariants worth preserving when changing this code:

- **Introspection must never fail open.** A network error or upstream 5xx is a
  `503`, never a `200`. Verification uses introspection rather than local JWT
  signature checking specifically so revocation takes effect immediately, which
  is why there is no response cache — only in-flight deduplication.
- **The `aud` check is mandatory.** Without it any token from the realm would
  work. The realm export ships a `no-audience-cli` client so this stays tested.
- **Secrets never reach logs**, at any level, and are never returned by any
  endpoint except `provision`, once.
- **Layer ordering in `routes::create_router` is load-bearing.** axum runs the
  last-added layer first, so authentication must be added after the rate limiter
  — the limiter keys on the subject authentication inserts.

### Keycloak specifics that cost time to rediscover

- Keycloak does **not** expand `${env.VAR}` inside an imported realm file; it
  stores the placeholder as the literal password. The `keycloak-realm` init
  container in compose renders the template with `sed` instead.
- Introspection responses only carry claims from mappers that set
  `introspection.token.claim: true`. Without the explicit `subject` and
  `realm-roles` mappers in the `totp-access` scope, the response has no `sub`
  and no roles at all.
- `KC_HOSTNAME` plus `KC_HOSTNAME_BACKCHANNEL_DYNAMIC` are both required: tokens
  must carry the issuer the host sees, while the server reaches Keycloak by its
  compose hostname. Without this every token introspects as inactive.

## CI

`.github/workflows/rust.yml`:

1. **build_server**: build, `cargo fmt --check`, clippy with `-D warnings`, tests.
2. **integration_test_server**: the `#[ignore]`d testcontainers tests.
3. **build_client_core**: builds/tests/lints `lock-client`'s `core` crate on
   the host.
4. **build_client_firmware**: builds/lints the ESP32-C6 firmware (from within
   `lock-client/firmware/`, per the toolchain note above) and uploads the
   resulting ELF as an artifact.
5. **simulate_client_firmware**: checks out `mikroverk/esp32sim` at a pinned
   commit (`ESP32SIM_REF`), builds `esp32sim-c6`, fetches and SHA-256-verifies
   the mask ROM, installs a pinned `espflash`, and runs
   `lock-client/firmware/sim/run_sim_tests.py` — see that directory's
   README.md for what it tests and how to bump the pinned versions.
