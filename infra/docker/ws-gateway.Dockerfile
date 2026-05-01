# Multi-stage build for attest WebSocket trace gateway (Phase 8).

FROM rust:1.95-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    libcurl4-openssl-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/
COPY tests/ tests/
COPY tools/ tools/

RUN cargo build --release -p ws-gateway

FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/ws-gateway /usr/local/bin/ws-gateway

ENV ATTEST_WS_GATEWAY_PORT=4500
ENV KAFKA_BROKERS=redpanda:9092

EXPOSE 4500

ENTRYPOINT ["ws-gateway"]
