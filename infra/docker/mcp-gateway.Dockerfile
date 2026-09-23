# Multi-stage build for attest-mcp-gateway.

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

RUN cargo build --release --bin attest-mcp-gateway

# ── Runtime image ──────────────────────────────────────────────────────────
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/attest-mcp-gateway /usr/local/bin/attest-mcp-gateway

ENV MCP_GATEWAY_PORT=4242
ENV CONTROL_PLANE_URL=http://control-plane:8080
ENV ATTEST_LOG_PATH=/data/attestations.ndjson

EXPOSE 4242

ENTRYPOINT ["attest-mcp-gateway"]
