# Attest

Attest is a streaming-first, agent-aware security operations platform built for cloud-native security teams. Events flow from cloud sources (CloudTrail, Okta, Entra ID) through an OCSF normalizer, into Redpanda, and are continuously aggregated by RisingWave materialized views. A REST control-plane API exposes recent events and 30-day entity baselines in real time. Parquet files are committed to MinIO via Apache Iceberg and queried at scale by ClickHouse.

## Documentation

| File | What it covers |
|---|---|
| `docs/README.md` | Product and architecture overview |
| `docs/01_PRD.md` | Product requirements document |
| `docs/02_Architecture.md` | System architecture |
| `docs/07_Stack_Revised.md` | Canonical tech stack |
| `docs/10_Build_Order.md` | Phase-by-phase implementation sequence |
| `docs/11_Repo_Structure.md` | Repository layout |

---

## What's Built

### Phase 1: Streaming Substrate

```
CloudTrail JSON
      │
      ▼ POST /ingest
attest-collector :4000   ──── OCSF JSON ───▶  Redpanda  (topic: cloudtrail)
      │                                             │
      │ GET /healthz                                │ Kafka source
      ▼                                             ▼
(health check)                            RisingWave :4566
                                           ├── entity_baselines (mat. view)
                                           └── recent_events    (mat. view)
                                                     ▲
                                          attest-control-plane :8080
                                           ├── GET /healthz
                                           ├── GET /v1/events/recent?id=<uuid>
                                           └── GET /v1/baselines/user/:name
```

### Phase 2: Iceberg Warm Tier

```
Redpanda (topic: cloudtrail)
      │
      │ Kafka consumer
      ▼
attest-storage-iceberg
      │ Arrow → Parquet (batched every 1 000 events or 30 s)
      ▼
MinIO (bucket: attest-warm) — Parquet files partitioned by date + tenant_id
      │
      │ s3() table function
      ▼
ClickHouse :8123
      ▲
attest-control-plane :8080
      └── POST /v1/warm/query  { "sql": "SELECT ..." }
```

### Crates

| Crate | Purpose |
|---|---|
| `crates/attest-common` | OCSF 1.3 types: `OcsfEvent`, `AuthenticationEvent` (class 3002), `CloudActivityEvent` (class 6003) |
| `crates/attest-collector` | Edge collector — CloudTrail JSON → OCSF normalizer → Redpanda + axum HTTP ingest |
| `crates/attest-control-plane` | Axum 0.8 REST API — RisingWave (hot tier) + ClickHouse (warm tier) |
| `crates/attest-storage-iceberg` | Kafka consumer → Arrow/Parquet writer → MinIO via `object_store` |

### Web App

The Next.js workbench at `apps/workbench/` is wired to the live backend:

| Route | Behaviour |
|---|---|
| `GET /api/events` | Proxies control-plane `/v1/events/recent`; falls back to mock if offline |
| `GET /api/baselines/user/[name]` | Proxies control-plane `/v1/baselines/user/:name` |
| `POST /api/warm/query` | Proxies control-plane `/v1/warm/query` (ClickHouse) |
| `/workbench/queue` | Live alert queue — real OCSF events when backend is up, mock otherwise |
| `/workbench/cases/[id]` | Case detail enriched with live event + baseline data |

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | 1.95+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Docker + Compose | 24+ | [docker.com](https://docs.docker.com/get-docker/) |
| Bun | 1.3+ | `curl -fsSL https://bun.sh/install \| bash` |
| Make | any | pre-installed on macOS/Linux |

---

## Quick Start

### 1. Start infrastructure

```sh
make dev-up
```

Starts Redpanda (`:9092`, `:19092`), RisingWave (`:4566`), Postgres (`:5432`), MinIO (`:9000`, console `:9001`), and ClickHouse (`:8123`) in Docker. MinIO bucket `attest-warm` is created automatically.

### 2. Start the full platform stack

```sh
make dev-up-platform
```

Builds and starts `attest-collector` (`:4000`), `attest-control-plane` (`:8080`), and `attest-storage-iceberg` using the multi-stage Dockerfiles in `infra/docker/`.

### 3. Verify services are healthy

```sh
curl http://localhost:4000/healthz    # {"status":"ok"}
curl http://localhost:8080/healthz    # {"status":"ok"}
```

### 4. Start the workbench UI

```sh
cd apps/workbench
bun install
bun run dev
# Open http://localhost:3000
```

The queue page badge will show **"Live — control-plane connected"** when the backend is reachable.

---

## Testing

### Unit Tests (no Docker required)

```sh
cargo test --workspace --lib
```

Covers OCSF type round-trips, CloudTrail normalizer, and serialization.

### CLI — Ingest a CloudTrail file

```sh
cargo build --release --bin attest-collector

./target/release/attest-collector \
  --kafka-brokers localhost:19092 \
  --tenant-id local-dev \
  ingest-file path/to/cloudtrail.json
```

### CLI — HTTP Ingest (curl)

```sh
curl -s -X POST http://localhost:4000/ingest \
  -H 'Content-Type: application/json' \
  -d '{
    "Records": [{
      "eventName": "ConsoleLogin",
      "eventTime": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
      "awsRegion": "us-west-2",
      "recipientAccountId": "123456789012",
      "userIdentity": {
        "type": "IAMUser",
        "userName": "alice@example.com",
        "arn": "arn:aws:iam::123456789012:user/alice",
        "accountId": "123456789012"
      }
    }]
  }'
# {"event_ids":["<uuid>"]}
```

### CLI — Query the control-plane API

```sh
# Recent event by ID
curl "http://localhost:8080/v1/events/recent?id=<event-uuid>"

# 30-day baseline for a user
curl "http://localhost:8080/v1/baselines/user/alice@example.com"
# {"tenant_id":"local-dev","actor_user_name":"alice@example.com","regions_seen_30d":["us-west-2"],...}

# Warm query via ClickHouse (Phase 2)
curl -s -X POST http://localhost:8080/v1/warm/query \
  -H 'Content-Type: application/json' \
  -d '{"sql": "SELECT cloud_region, count(*) FROM s3(\"http://minio:9000/attest-warm/cloudtrail/**/*.parquet\", \"minioadmin\", \"minioadmin\", \"Parquet\") GROUP BY cloud_region"}'
```

### E2E Tests (require running stack)

```sh
# Phase 1 — streaming substrate
make e2e-phase1
# Posts a ConsoleLogin, polls recent_events (5 s), polls entity_baselines (30 s).

# Phase 2 — Iceberg warm tier
make e2e-phase2
# Seeds 10 000 events, waits for Iceberg commit (≤ 60 s), asserts ClickHouse count + aggregate (≤ 30 s).
```

Both targets require `make dev-up-platform` to be running first.

---

## Project Structure

```
Attest/
├── crates/
│   ├── attest-common/          # OCSF 1.3 types (Phase 1)
│   ├── attest-collector/       # CloudTrail → Redpanda edge collector (Phase 1)
│   ├── attest-control-plane/   # axum 0.8 REST API — hot + warm tier (Phase 1 + 2)
│   └── attest-storage-iceberg/ # Kafka → Parquet → MinIO writer (Phase 2)
├── apps/
│   └── workbench/              # Next.js 15 marketing site + SOC workbench UI
├── infra/
│   ├── clickhouse/             # ClickHouse config (S3/MinIO disk)
│   ├── docker/                 # Multi-stage Dockerfiles (collector, control-plane, storage-iceberg)
│   ├── risingwave/             # RisingWave DDL (phase1_baseline.sql)
│   └── terraform/              # IaC (Phase 5+)
├── tests/
│   └── e2e-tests/              # E2E tests (ATTEST_E2E=1 required)
│       └── tests/
│           ├── phase1_streaming.rs
│           └── phase2_iceberg.rs
├── docs/                       # Source-of-truth documentation
├── docker-compose.yml          # Local dev stack
└── Makefile                    # Convenience targets
```

---

## Make Targets

| Target | What it does |
|---|---|
| `make dev-up` | Start core infra (Redpanda, RisingWave, Postgres, MinIO + init, ClickHouse) |
| `make dev-up-platform` | Start core infra + collector + control-plane + storage-iceberg |
| `make dev-down` | Stop all containers |
| `make e2e-phase1` | Run Phase 1 E2E test (requires `ATTEST_E2E=1` + running stack) |
| `make e2e-phase2` | Run Phase 2 E2E test (requires `ATTEST_E2E=1` + running stack) |
| `make fmt` | `cargo fmt --all` |
| `make lint` | `cargo clippy` + `bun run lint` |
| `make smoke` | Quick smoke check — cargo test + bun test + pytest |
| `make dev-up-llm` | Start core infra + llama.cpp (requires Qwen GGUF volume-mounted) |
