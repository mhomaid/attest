# Multi-stage build for workbench-api (attestation trace reader).

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

RUN cargo build --release -p workbench-api

FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/workbench-api /usr/local/bin/workbench-api

ENV WORKBENCH_API_PORT=4400
ENV PORT=4400
ENV ATTEST_LOG_PATH=/data/attestations.ndjson

EXPOSE 4400

ENTRYPOINT ["workbench-api"]
