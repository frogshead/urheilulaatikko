# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Two independent Rust crates (not a cargo workspace — each has its own
`Cargo.toml` and is built with `--manifest-path`):

- **lock-client**: TOTP code generation for the offline lock, using `totp_embed`
  with SHA-1 for `no_std` compatibility. Cross-compiled for Raspberry Pi.
- **totp-server**: an axum service that issues one-time PINs to users who have
  authenticated against Keycloak. See `totp-server/README.md` for the API,
  configuration, and development environment.

The two ends never talk to each other. The lock is offline; the user carries the
PIN across by typing it in. The only thing they share is the TOTP secret.

**Algorithms are aligned:** both sides use SHA-1, 6 digits, 30-second steps
(RFC 6238 / Google Authenticator profile — see `lock-client/src/main.rs` and
`totp-server/src/totp/mod.rs`), so the same secret produces the same PIN on
both ends.

## Build and Test Commands

```bash
# Build
cargo build --manifest-path ./lock-client/Cargo.toml
cargo build --manifest-path ./totp-server/Cargo.toml

# Test (server: unit tests + wiremock-backed introspection tests, no Docker)
cargo test --manifest-path ./lock-client/Cargo.toml
cargo test --manifest-path ./totp-server/Cargo.toml

# Server integration tests against real Postgres and Keycloak containers.
# These are #[ignore]d by default and need a working Docker daemon.
cargo test --manifest-path ./totp-server/Cargo.toml --test e2e_integration -- --ignored

# Lints that CI enforces
cargo fmt --manifest-path ./totp-server/Cargo.toml --check
cargo clippy --manifest-path ./totp-server/Cargo.toml --all-targets -- -D warnings
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

### Cross-compilation for Raspberry Pi

```bash
cargo binstall --no-confirm cross
cross build --release --manifest-path ./lock-client/Cargo.toml --target armv7-unknown-linux-gnueabihf
```

Binary lands at `lock-client/target/armv7-unknown-linux-gnueabihf/release/lock-client`.

## Architecture Notes

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
3. **build_client**: build and test natively, cross-compile for armv7, upload
   the Raspberry Pi binary as an artifact.
