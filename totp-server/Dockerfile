# ---- build ----
FROM rust:1.89-slim AS builder

WORKDIR /build

# Dependencies first, so editing sources does not re-download the registry.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo 'fn main() {}' > src/main.rs \
    && echo '' > src/lib.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
COPY migrations ./migrations
# Touch so cargo does not reuse the placeholder build artefacts.
RUN touch src/main.rs src/lib.rs && cargo build --release

# ---- runtime ----
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 totp

WORKDIR /app
COPY --from=builder /build/target/release/totp-server /usr/local/bin/totp-server
# The migrations are embedded in the binary by sqlx::migrate!; this copy is only
# here so they can be inspected or replayed by hand inside the container.
COPY --from=builder /build/migrations ./migrations

USER totp
EXPOSE 3000

# No curl in the runtime image; bash's /dev/tcp is enough for a liveness probe.
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=5 \
    CMD bash -c 'exec 3<>/dev/tcp/localhost/3000 && echo -e "GET /healthz HTTP/1.1\nHost: localhost\nConnection: close\n\n" >&3 && grep -q "\"status\":\"ok\"" <&3'

CMD ["totp-server"]
