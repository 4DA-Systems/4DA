# ---- Build stage ----
FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/

RUN cargo build --release --locked 2>/dev/null || cargo build --release

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

ENV DATABASE_URL="sqlite:/app/data/relay.db?mode=rwc" \
    PORT=8443 \
    RUST_LOG="relay=info,tower_http=info"

EXPOSE 8443

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8443/health || exit 1

ENTRYPOINT ["/app/fourda-relay"]
