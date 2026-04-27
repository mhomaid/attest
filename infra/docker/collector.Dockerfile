# Multi-stage build for attest-collector.
# Stage 1: build static binary using the official Rust image.
# Stage 2: minimal runtime image — target is <50 MB.

FROM rust:1.95-slim AS builder

# cmake and libssl are needed by rdkafka's cmake-build feature.
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    libcurl4-openssl-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests first for better layer caching.
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/
COPY tests/ tests/

# Build only the collector binary in release mode.
RUN cargo build --release --bin attest-collector

# ── Runtime image ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/attest-collector /usr/local/bin/attest-collector

ENV PORT=4000
ENV KAFKA_BROKERS=redpanda:9092
ENV TENANT_ID=default

EXPOSE 4000

ENTRYPOINT ["attest-collector", "serve"]
