# ---- Build stage ----
# Matches the repo's pinned toolchain (rust-toolchain.toml, 1.95.0). 1.82 could
# not build this lockfile at all — `time 0.3.47` (jsonwebtoken -> simple_asn1)
# declares rust-version 1.88 — a failure the `|| cargo build --release` fallback
# below used to hide behind a re-resolve that could not have succeeded either.
FROM rust:1.95-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/

# --locked is NOT optional. The previous form was
#   cargo build --release --locked 2>/dev/null || cargo build --release
# whose fallback silently re-resolved the dependency graph whenever the
# lockfile was stale, missing, or unsatisfiable — shipping an image built
# from versions nobody audited, with the failure swallowed by 2>/dev/null.
# If this line fails now, fix Cargo.lock; do not route around it.
RUN cargo build --release --locked

# ---- Runtime stage ----
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

RUN groupadd --system relay && \
    useradd --system --gid relay --create-home relay

WORKDIR /app

COPY --from=builder /build/target/release/fourda-relay /app/fourda-relay

RUN mkdir -p /app/data && chown -R relay:relay /app

USER relay

# BIND_ADDR must be 0.0.0.0 inside a container. main.rs defaults it to 127.0.0.1,
# which is the right default for a bare-metal run but binds only the container's
# own loopback here — published ports resolve to nothing while the healthcheck,
# which curls localhost from inside the same netns, still passes. The container
# looked healthy and was unreachable.
ENV DATABASE_URL="sqlite:/app/data/relay.db?mode=rwc" \
    BIND_ADDR="0.0.0.0" \
    PORT=8443 \
    RUST_LOG="relay=info,tower_http=info"

EXPOSE 8443

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8443/health || exit 1

ENTRYPOINT ["/app/fourda-relay"]
