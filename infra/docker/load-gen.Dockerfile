# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.85-slim AS builder

RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    libcurl4-openssl-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/workbench-api/ apps/workbench-api/
COPY apps/ws-gateway/ apps/ws-gateway/
COPY tests/e2e-tests/ tests/e2e-tests/
COPY tools/load-gen/ tools/load-gen/

RUN cargo build --release -p attest-load-gen

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    libcurl4 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/attest-load-gen /usr/local/bin/attest-load-gen

ENV KAFKA_BROKERS=redpanda:9092
ENV LOAD_GEN_PORT=9100
ENV RUST_LOG=info

EXPOSE 9100

ENTRYPOINT ["attest-load-gen"]
