# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

WORKDIR /build

# Install OpenSSL dev headers (needed by reqwest's TLS)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies separately from source
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs
RUN cargo build --release 2>/dev/null; true
RUN rm src/main.rs

# Build the real binary
COPY src ./src
COPY keys ./keys
RUN touch src/main.rs && cargo build --release

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/schedula-license ./schedula-license
COPY --from=builder /build/keys ./keys

# /data is the persistent volume mount point on Fly.io
RUN mkdir -p /data

EXPOSE 8080

# All secrets passed via environment variables (see fly.toml [env] and secrets)
CMD ["./schedula-license", \
     "--port", "8080", \
     "--db-path", "/data/schedula-license.db", \
     "--private-key", "/app/keys/private.pem", \
     "--public-key",  "/app/keys/public.pem"]
