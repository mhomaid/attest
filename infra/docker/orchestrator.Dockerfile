# Multi-stage build for attest-orchestrator.

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
COPY agents/ agents/

RUN cargo build --release --bin attest-orchestrator

# ── Runtime image ──────────────────────────────────────────────────────────
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/attest-orchestrator /usr/local/bin/attest-orchestrator
COPY --from=builder /build/agents /agents/
COPY ml/triager/artifacts/ /artifacts/

ENV ORCHESTRATOR_PORT=4300
ENV ARTIFACTS_DIR=/artifacts
ENV CALIBRATION_URL=http://calibration-sidecar:5001
ENV ESCALATION_THRESHOLD=0.60
ENV NOVELTY_THRESHOLD=0.70
ENV ATTEST_LOG_PATH=/data/attestations.ndjson
ENV SYSTEM_PROMPT_PATH=/agents/triager/system_prompt_v1.md
ENV REVIEWER_PROMPT_PATH=/agents/triager/reviewer_prompt_v1.md
ENV INVESTIGATOR_PROMPT_PATH=/agents/investigator/system_prompt_v1.md
ENV MCP_GATEWAY_URL=http://mcp-gateway:4242

EXPOSE 4300

ENTRYPOINT ["attest-orchestrator"]
