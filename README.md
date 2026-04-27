# Attest

Attest is a streaming-first, agent-aware security operations platform built for cloud-native security teams. Events flow from cloud sources (CloudTrail, Okta, Entra ID) through an OCSF normalizer, into Redpanda, and are continuously aggregated by RisingWave materialized views. A REST control-plane API exposes recent events and 30-day entity baselines in real time.

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

## What's Built — Phase 1: Streaming Substrate

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

### Crates

| Crate | Purpose |
|---|---|
| `crates/attest-common` | OCSF 1.3 types: `OcsfEvent`, `AuthenticationEvent` (class 3002), `CloudActivityEvent` (class 6003), `Actor`, `User`, `Cloud`, `Severity` |
| `crates/attest-collector` | Edge collector binary — CloudTrail JSON → OCSF normalizer → Redpanda producer + axum HTTP ingest server |
| `crates/attest-control-plane` | Axum REST API — queries RisingWave materialized views via Postgres wire protocol; applies DDL on boot |

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | 1.82+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Docker + Compose | 24+ | [docker.com](https://docs.docker.com/get-docker/) |
| Bun | 1.3+ | `curl -fsSL https://bun.sh/install \| bash` |
| Make | any | pre-installed on macOS/Linux |

---

## Quick Start

### 1. Start infrastructure (Redpanda + RisingWave + Postgres + MinIO)

```sh
make dev-up
```

This starts Redpanda (`:9092`, `:9644`), RisingWave (`:4566`), Postgres (`:5432`), MinIO, and ClickHouse in Docker.

### 2. Start Phase 1 services (collector + control-plane)

```sh
make dev-up-platform
```

This builds and starts `attest-collector` (`:4000`) and `attest-control-plane` (`:8080`) using the multi-stage Dockerfiles in `infra/docker/`.

### 3. Verify both services are healthy

```sh
curl http://localhost:4000/healthz
# {"status":"ok"}

curl http://localhost:8080/healthz
# {"status":"ok"}
```

---

## Testing

### Unit Tests (no Docker required)

Runs all library tests including OCSF type round-trips and CloudTrail normalizer:

```sh
cargo test --workspace --lib
```

Expected output: all tests pass, including:
- `attest_common::ocsf::tests::authentication_event_round_trips`
- `attest_common::ocsf::tests::ocsf_event_enum_round_trips`
- `attest_collector::normalizer::tests::normalizes_console_login`
- `attest_collector::normalizer::tests::normalizes_failed_login`
- `attest_collector::normalizer::tests::normalizes_cloud_activity`

### CLI — Ingest a CloudTrail file

With infrastructure running (`make dev-up-platform`), ingest a local CloudTrail JSON file directly:

```sh
# Build the binary first
cargo build --release --bin attest-collector

# Ingest a CloudTrail file
./target/release/attest-collector \
  --kafka-brokers localhost:9092 \
  --tenant-id my-tenant \
  ingest-file path/to/cloudtrail.json
```

You can use this sample payload to test manually:

```sh
cat > /tmp/sample_cloudtrail.json << 'EOF'
{
  "Records": [{
    "eventName": "ConsoleLogin",
    "eventTime": "2024-01-15T10:30:00Z",
    "awsRegion": "us-west-2",
    "recipientAccountId": "123456789012",
    "userIdentity": {
      "type": "IAMUser",
      "userName": "alice@example.com",
      "arn": "arn:aws:iam::123456789012:user/alice",
      "accountId": "123456789012"
    }
  }]
}
EOF

./target/release/attest-collector \
  --kafka-brokers localhost:9092 \
  --tenant-id local-dev \
  ingest-file /tmp/sample_cloudtrail.json
```

### CLI — HTTP Ingest (curl)

```sh
curl -s -X POST http://localhost:4000/ingest \
  -H 'Content-Type: application/json' \
  -d '{
    "Records": [{
      "eventName": "ConsoleLogin",
      "eventTime": "2024-01-15T10:30:00Z",
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
# Look up an event by ID (replace with an ID from the /ingest response)
curl "http://localhost:8080/v1/events/recent?id=<event-uuid>"

# Get alice's 30-day baseline (regions seen, event count, last seen)
curl "http://localhost:8080/v1/baselines/user/alice@example.com"
# {"tenant_id":"local-dev","actor_user_name":"alice@example.com","regions_seen_30d":["us-west-2"],"event_count_30d":1,"last_seen":"..."}
```

### E2E Test (requires running stack)

The E2E test posts a ConsoleLogin event, polls for it in `recent_events` (5 s timeout), and polls alice's baseline for `us-west-2` (10 s timeout):

```sh
# Start the full stack first
make dev-up-platform

# Run the E2E test (guarded by ATTEST_E2E=1)
ATTEST_E2E=1 cargo test --test phase1_streaming -- --nocapture
```

### UI — Workbench (Next.js)

The marketing home page and workbench shell are available locally:

```sh
# Install JS dependencies
bun install

# Start the workbench dev server
bun run dev --filter @attest/workbench
# Open http://localhost:3000
```

---

## Project Structure

```
Attest/
├── crates/
│   ├── attest-common/          # OCSF types (Phase 1)
│   ├── attest-collector/       # CloudTrail → Redpanda edge collector (Phase 1)
│   ├── attest-control-plane/   # axum REST API (Phase 1)
│   └── …                       # future crates per build-order phase
├── apps/
│   └── workbench/              # Next.js marketing + workbench UI
├── infra/
│   ├── docker/                 # Multi-stage Dockerfiles
│   ├── risingwave/             # RisingWave DDL (phase1_baseline.sql)
│   └── terraform/              # IaC (Phase 5+)
├── tests/
│   └── e2e-tests/              # Integration tests (ATTEST_E2E=1 to run)
├── docs/                       # Source-of-truth documentation
├── docker-compose.yml          # Local dev stack
└── Makefile                    # Convenience targets
```

---

## Make Targets

| Target | What it does |
|---|---|
| `make dev-up` | Start core infra (Redpanda, RisingWave, Postgres, MinIO, ClickHouse) |
| `make dev-up-platform` | Start core infra + collector + control-plane (full Phase 1 stack) |
| `make dev-down` | Stop all containers |
| `make smoke` | Quick smoke check against running services |
| `make dev-up-llm` | Start core infra + llama.cpp sidecar |

The JavaScript workspace uses Bun, matching `docs/12_Workbench.md`. Python commands run through `uv` so ML tooling does not require globally installed packages.
