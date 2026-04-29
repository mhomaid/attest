## ── builder ────────────────────────────────────────────────────────────────
FROM rust:1.95-slim AS builder

RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    libssl-dev \
    libcurl4-openssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependency compilation by pre-copying manifests.
COPY Cargo.toml Cargo.lock ./
COPY crates/attest-common/Cargo.toml         crates/attest-common/Cargo.toml
COPY crates/attest-collector/Cargo.toml      crates/attest-collector/Cargo.toml
COPY crates/attest-control-plane/Cargo.toml  crates/attest-control-plane/Cargo.toml
COPY crates/attest-storage-iceberg/Cargo.toml crates/attest-storage-iceberg/Cargo.toml
COPY apps/workbench-api/Cargo.toml           apps/workbench-api/Cargo.toml
COPY apps/ws-gateway/Cargo.toml              apps/ws-gateway/Cargo.toml
COPY tests/e2e-tests/Cargo.toml              tests/e2e-tests/Cargo.toml
COPY tools/load-gen/Cargo.toml               tools/load-gen/Cargo.toml

RUN mkdir -p \
    crates/attest-common/src \
    crates/attest-collector/src \
    crates/attest-control-plane/src \
    crates/attest-storage-iceberg/src \
    apps/workbench-api/src \
    apps/ws-gateway/src \
    tests/e2e-tests/src \
    tools/load-gen/src \
    && echo "fn main(){}" > crates/attest-common/src/main.rs \
    && echo "fn main(){}" > crates/attest-collector/src/main.rs \
    && echo "fn main(){}" > crates/attest-control-plane/src/main.rs \
    && echo "fn main(){}" > crates/attest-storage-iceberg/src/main.rs \
    && echo "fn main(){}" > apps/workbench-api/src/main.rs \
    && echo "fn main(){}" > apps/ws-gateway/src/main.rs \
    && echo "fn main(){}" > tools/load-gen/src/main.rs \
    && touch tests/e2e-tests/src/lib.rs

RUN cargo build --release -p attest-storage-iceberg 2>&1 || true

COPY crates/ crates/
COPY apps/workbench-api/src apps/workbench-api/src
COPY apps/ws-gateway/src    apps/ws-gateway/src
COPY tests/                 tests/

RUN cargo build --release -p attest-storage-iceberg

## ── runtime ─────────────────────────────────────────────────────────────────
FROM debian:trixie-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libcurl4 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/attest-storage-iceberg /usr/local/bin/

ENTRYPOINT ["/usr/local/bin/attest-storage-iceberg"]
