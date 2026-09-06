# TOTP server

Issues one-time PIN codes to users who have authenticated against Keycloak.

A user logs in to Keycloak, presents the resulting access token here, and gets
back the current TOTP code. They type that code into an offline lock by hand.
**This server never talks to the lock** — the lock is a standalone device with
no network connection, and the shared TOTP secret is the only thing the two ends
have in common.

```
  user ──login──▶ Keycloak
  user ──Bearer token──▶ totp-server ──introspect──▶ Keycloak
  user ◀──── PIN ────── totp-server
  user ──types PIN──▶ lock   (offline, no network)
```

## What it does

1. Takes a request with `Authorization: Bearer <JWT>`.
2. Verifies the token with Keycloak using **RFC 7662 introspection** rather than
   local signature checking, so a revoked token stops working immediately.
3. Looks up the TOTP secret for the token's `sub`, decrypting it on the way out
   of the database.
4. Computes the current RFC 6238 code and returns it.
5. Rate-limits per user.

## API

### `POST /v1/totp/request`

Header: `Authorization: Bearer <jwt>`

```json
{ "pin": "482913", "valid_for_seconds": 21 }
```

| Status | Meaning |
| ------ | ------- |
| `200`  | PIN issued |
| `401`  | Token missing, expired, or `active: false` |
| `403`  | Token valid, but its `aud` does not name this server |
| `404`  | No TOTP secret registered for this user |
| `429`  | Rate limit exceeded |
| `503`  | Keycloak unreachable — the request is refused, never waved through |

### `POST /v1/totp/provision`

Admin only: requires the `totp-admin` realm role. Body `{"subject": "<keycloak user id>"}`.

```json
{ "secret_base32": "...", "otpauth_url": "otpauth://totp/..." }
```

The plaintext secret is returned **once, here, and nowhere else**. It is stored
encrypted, no other endpoint exposes it, and it is never written to a log line.
Calling this again for the same subject replaces the secret and invalidates the
old one.

### `GET /healthz`

Unauthenticated. Reports the TOTP parameters so a lock's configuration can be
cross-checked against the server's:

```json
{ "status": "ok", "algorithm": "SHA1", "digits": 6, "step_seconds": 30, "skew_steps": 1 }
```

## Running the development environment

```bash
cp .env.example .env
# Generate a real encryption key and pick test passwords:
echo "TOTP_ENCRYPTION_KEY=base64:$(openssl rand -base64 32)" >> .env
docker compose up -d --build
./scripts/e2e.sh
```

`e2e.sh` logs in as the admin test user, provisions a secret for the regular test
user, requests a PIN, and checks that unauthenticated and non-admin requests are
refused.

The compose stack is Postgres (one instance backing both Keycloak and this
server), Keycloak with the `lock-demo` realm imported, and the server itself. If
one of the default host ports is already taken, override `POSTGRES_PORT`,
`KEYCLOAK_PORT`, or `TOTP_SERVER_PORT` in `.env`.

### Test users

Created by the realm import, with passwords taken from `.env`:

| User        | Password              | Role |
| ----------- | --------------------- | ---- |
| `testuser`  | `TEST_USER_PASSWORD`  | requests PINs |
| `testadmin` | `TEST_ADMIN_PASSWORD` | also provisions secrets (`totp-admin`) |

Keycloak's admin console is at `http://localhost:8080` (`admin` / `admin`).

### Doing it by hand

```bash
KC=http://localhost:8080/realms/lock-demo

TOKEN=$(curl -s -X POST $KC/protocol/openid-connect/token \
  -d grant_type=password -d client_id=lock-cli \
  -d username=testuser -d password="$TEST_USER_PASSWORD" | jq -r .access_token)

curl -s -X POST http://localhost:3000/v1/totp/request \
  -H "Authorization: Bearer $TOKEN" | jq
```

Introspection happens inside the server; the caller never sees it.

## Configuration

See `.env.example`. The ones that matter:

| Variable | Default | Notes |
| -------- | ------- | ----- |
| `DATABASE_URL` | — | required |
| `KEYCLOAK_ISSUER` | — | realm base URL, e.g. `http://keycloak:8080/realms/lock-demo` |
| `TOTP_SERVER_KEYCLOAK_CLIENT_ID` / `_SECRET` | — | confidential client used for introspection |
| `TOTP_ENCRYPTION_KEY` | — | 32 bytes, base64, optional `base64:` prefix |
| `TOTP_EXPECTED_AUDIENCE` | `totp-server` | must appear in the token's `aud` |
| `TOTP_ADMIN_ROLE` | `totp-admin` | realm role required to provision |
| `TOTP_SKEW_STEPS` | `1` | reported on `/healthz`; see below |
| `RATE_LIMIT_MAX_REQUESTS` | `6` | per subject |
| `RATE_LIMIT_WINDOW_SECONDS` | `300` | per subject |

### `TOTP_SKEW_STEPS`

The server always generates the code for the *current* 30-second step. It has no
way to know how far the lock's real-time clock has drifted, so drift is absorbed
on the lock's side: the lock accepts a code from this many steps either way.
The value is configured here only so that `/healthz` can report it and the two
configurations can be compared without reading either one's environment.

## Security notes

- **Introspection never fails open.** If Keycloak is unreachable or errors, the
  request gets a `503`. A token is never assumed valid.
- **The `aud` check is mandatory.** Without it, any token issued anywhere in the
  realm — including one minted for an unrelated client — would unlock a PIN. The
  realm export ships a `no-audience-cli` client purely so this can be tested end
  to end.
- **Secrets never reach the logs**, at any level. `Crypto`'s `Debug` output is
  redacted so the key cannot leak through a `{:?}`.
- **`TOTP_ENCRYPTION_KEY` belongs in a vault or KMS in production**, not in an
  environment variable. The `.env` route is a development convenience. Rotating
  the key requires re-encrypting every row in `totp_secrets`; there is no key
  versioning yet.
- Nonces are drawn fresh per encryption and stored alongside the ciphertext, so
  no nonce is ever reused under one key.
- Traffic between this server and Keycloak is plain HTTP in the dev compose
  setup. In production it should be TLS, and mTLS is worth considering since the
  client secret is otherwise the only thing authenticating the introspection
  call.

## Audit trail

Every PIN request lands in `request_log` with one of `issued`,
`denied_no_secret`, `denied_inactive_token`, `rate_limited`, or `provisioned`.
The table holds no PINs and no secrets. `subject` is null for
`denied_inactive_token`, because a token rejected by introspection carries no
usable subject.

```sql
select result, count(*) from request_log group by result;
```

## Tests

```bash
cargo test                                        # unit + wiremock, no Docker
cargo test --test e2e_integration -- --ignored    # real Keycloak + Postgres
```

- **Unit tests** cover the RFC 6238 vectors, AES-GCM round-tripping and
  tampering, config parsing, bearer-header parsing, and the rate-limit key
  extractor.
- **`tests/introspection.rs`** drives the introspection client against a
  wiremock Keycloak: active and inactive tokens, both `aud` encodings, expiry,
  upstream 5xx, unreachable host, malformed responses, and the in-flight
  deduplication.
- **`tests/e2e_integration.rs`** starts real Postgres and Keycloak containers
  from the same realm export compose uses, and checks the issued PIN against an
  independent RFC 6238 implementation.

## Layout

```
totp-server/
├── src/
│   ├── main.rs                    thin binary; everything else is in the lib
│   ├── config.rs                  env parsing and validation
│   ├── crypto.rs                  AES-256-GCM at rest
│   ├── db.rs                      pool and migrations
│   ├── error.rs                   AppError -> HTTP status, with cause logging
│   ├── rate_limit.rs              per-subject key extractor
│   ├── state.rs                   shared AppState
│   ├── auth/
│   │   ├── introspection.rs       RFC 7662 client, single-flight dedup
│   │   └── middleware.rs          bearer extraction, audit of denials
│   ├── routes/{health,totp,provision}.rs
│   └── totp/
│       ├── mod.rs                 RFC 6238 generation
│       └── secret_store.rs        encrypted storage + audit log
├── migrations/0001_init.sql
├── docker/
│   ├── keycloak/                  realm template + render script
│   └── postgres/init-multi-db.sh
└── scripts/e2e.sh
```
