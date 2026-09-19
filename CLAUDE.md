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
    **ESP32-C3-DevKit-RUST-2** (ESP32-C3-MINI-1, RISC-V,
    `riscv32imc-unknown-none-elf`), built on `esp-hal`. Wires `core`'s logic
    to real GPIO interrupts, a monotonic timer, and a relay output.
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

**Toolchain note:** `lock-client/firmware`'s `esp-hal` dependency stack is
pinned to exact versions (see its Cargo.toml) because newer releases require
rustc 1.95, ahead of the 1.89 toolchain this was built and verified against.
Also note: firmware must be built by `cd`-ing into `lock-client/firmware` and
running plain `cargo build`/`clippy`/`fmt` there — Cargo resolves
`.cargo/config.toml` (which sets the RISC-V target and a linker flag
`esp-hal`'s generated linker script needs) relative to the *working
directory*, not `--manifest-path`, so invoking it from elsewhere silently
skips that config and fails to build or link.

**Flashed and boot-verified on a real ESP32-C3-DevKit-RUST-2** (`/dev/ttyACM0`
on Linux). Two things only surfaced at that step, not from `cargo build` —
see `lock-client/README.md` for the details: the firmware needs
`esp_bootloader_esp_idf::esp_app_desc!()` in `main.rs` or `espflash` refuses
to flash it, and `espflash` itself needed upgrading to 4.5.0 (the newest
version still compatible with rustc 1.89) since 4.3.0 failed the flash step.
Only boot has been verified this way — the Wiegand/TOTP/relay path is still
only exercised by `core`'s unit tests, not against a real keypad.

## Build and Test Commands

```bash
# Build (lock-client: builds `core`, the workspace default member — host-only)
cargo build --manifest-path ./lock-client/Cargo.toml
cargo build --manifest-path ./totp-server/Cargo.toml

# Test (lock-client/core: pure logic; server: unit + wiremock introspection
# tests. Neither needs Docker or an embedded toolchain)
cargo test --manifest-path ./lock-client/Cargo.toml
cargo test --manifest-path ./totp-server/Cargo.toml

# ESP32-C3 firmware — must run from within the directory, see the toolchain
# note above
( cd lock-client/firmware && rustup target add riscv32imc-unknown-none-elf && cargo build --release )

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
4. **build_client_firmware**: builds/lints the ESP32-C3 firmware (from within
   `lock-client/firmware/`, per the toolchain note above) and uploads the
   resulting ELF as an artifact.
