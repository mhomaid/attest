# Multi-stage build for attest-orchestrator.

FROM rust:1.95-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    cmake \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/
COPY tests/ tests/

RUN cargo build --release --bin attest-orchestrator

# ── Runtime image ──────────────────────────────────────────────────────────
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/attest-orchestrator /usr/local/bin/attest-orchestrator

ENV ORCHESTRATOR_PORT=4300
ENV ARTIFACTS_DIR=/artifacts
ENV CALIBRATION_URL=http://calibration-sidecar:5001
ENV ESCALATION_THRESHOLD=0.60
ENV NOVELTY_THRESHOLD=0.70
ENV ATTEST_LOG_PATH=/data/attestations.ndjson

EXPOSE 4300

ENTRYPOINT ["attest-orchestrator"]
