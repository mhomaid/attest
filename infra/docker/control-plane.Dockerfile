# Multi-stage build for attest-control-plane.

FROM rust:1.98-slim AS builder

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
COPY infra/risingwave/ infra/risingwave/

RUN cargo build --release --bin attest-control-plane

# ── Runtime image ─────────────────────────────────────────────────────────
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/attest-control-plane /usr/local/bin/attest-control-plane

ENV PORT=8080
ENV RISINGWAVE_HOST=risingwave
ENV RISINGWAVE_PORT=4566

EXPOSE 8080

ENTRYPOINT ["attest-control-plane"]
