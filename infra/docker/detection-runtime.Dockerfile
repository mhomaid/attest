# ── builder ──────────────────────────────────────────────────────────────────
FROM rust:1.95-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential cmake libssl-dev libcurl4-openssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/workbench-api apps/workbench-api
COPY apps/ws-gateway apps/ws-gateway
COPY tests/ tests/
COPY tools/ tools/

RUN cargo build --release --bin attest-detection-runtime

# ── runtime ──────────────────────────────────────────────────────────────────
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 libcurl4 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/attest-detection-runtime /usr/local/bin/attest-detection-runtime

# Bake detection rules into the image so the service works on Railway
# (no local volume mounts available). Override RULES_DIR at runtime if needed.
COPY detections/ /rules/

ENV RULES_DIR=/rules

ENTRYPOINT ["/usr/local/bin/attest-detection-runtime"]
