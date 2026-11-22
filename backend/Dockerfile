# syntax=docker/dockerfile:1.7

# ── Stockfish 17 (official source, compiled for the target architecture) ─────
FROM debian:bookworm-slim AS stockfish
ARG TARGETARCH
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl make g++ \
    && rm -rf /var/lib/apt/lists/*
RUN set -eux; \
    curl -fsSL https://github.com/official-stockfish/Stockfish/archive/refs/tags/sf_17.tar.gz -o /tmp/sf17.tar.gz; \
    tar -xzf /tmp/sf17.tar.gz -C /tmp; \
    case "${TARGETARCH:-amd64}" in \
        amd64) SF_ARCH="x86-64-avx2" ;; \
        arm64) SF_ARCH="armv8" ;; \
        *) SF_ARCH="x86-64-sse41-popcnt" ;; \
    esac; \
    make -C /tmp/Stockfish-sf_17/src -j"$(nproc)" build ARCH="${SF_ARCH}"; \
    install -m 0755 /tmp/Stockfish-sf_17/src/stockfish /usr/local/bin/stockfish; \
    /usr/local/bin/stockfish uci | head -n 1

# ── Rust API ─────────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY backend/Cargo.toml backend/Cargo.toml
RUN mkdir -p backend/src \
    && printf 'fn main() {}\n' > backend/src/main.rs \
    && printf '' > backend/src/lib.rs \
    && cargo build --release --package eloforge-api || true

COPY backend/ ./backend/
RUN touch backend/src/main.rs backend/src/lib.rs \
    && cargo build --release --package eloforge-api

# ── Runtime ──────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libstdc++6 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 eloforge \
    && useradd --system --uid 10001 --gid eloforge --create-home --home-dir /home/eloforge eloforge

COPY --from=builder /app/target/release/eloforge-api /usr/local/bin/eloforge-api
COPY --from=stockfish /usr/local/bin/stockfish /usr/local/bin/stockfish

ENV PORT=8080 \
    STOCKFISH_PATH=/usr/local/bin/stockfish \
    LOG_FORMAT=json \
    COOKIE_SECURE=true

USER 10001:10001
EXPOSE 8080

HEALTHCHECK --interval=20s --timeout=5s --start-period=25s --retries=5 \
    CMD curl -fsS "http://127.0.0.1:${PORT}/api/healthz" >/dev/null || exit 1

ENTRYPOINT ["/usr/local/bin/eloforge-api"]
