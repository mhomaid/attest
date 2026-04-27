# Attest

Attest is a streaming-first, agent-aware security operations platform built for cloud-native security teams. Events flow from cloud sources (CloudTrail, Okta, Entra ID) through an OCSF normalizer into Redpanda, are continuously aggregated by RisingWave materialized views, persisted as Parquet files in MinIO via Apache Iceberg, and queried at scale by ClickHouse — all exposed through a REST control-plane and a live Next.js SOC workbench.

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

### Phase 1 — Streaming Substrate

**The problem it solves:** Raw CloudTrail JSON is vendor-specific and inconsistent. Before you can detect anything, every event needs to be normalized to a common schema, streamed reliably, and made queryable in real time for both individual event lookup and aggregate behavioral baselines.

#### Pillar 1 — `attest-collector` (Edge Ingest + Normalization)

Accepts raw CloudTrail JSON via `POST /ingest`, normalizes every record into the **OCSF 1.3** schema (Open Cybersecurity Schema Framework), then produces a flat JSON `FlatEvent` to Redpanda. Shared OCSF types live in `crates/attest-common` so every service agrees on the same event shape.

```
POST /ingest  (CloudTrail JSON)
      ↓  attest-collector :4000
      ↓  OCSF normalizer → FlatEvent
      ↓  rdkafka producer
Redpanda :9092  (topic: cloudtrail)
```

#### Pillar 2 — RisingWave Materialized Views (Hot Tier)

RisingWave subscribes to the `cloudtrail` Kafka topic and maintains two always-fresh materialized views with no polling lag:

- **`recent_events`** — last N events per `event_id`, queryable in under a second
- **`entity_baselines`** — rolling 30-day aggregate of regions, services, and activity counts per user

```
Redpanda (topic: cloudtrail)
      ↓  Kafka source
RisingWave :4566
  ├── recent_events    (mat. view — event lookup by ID)
  └── entity_baselines (mat. view — 30d rolling user profile)
```

#### Pillar 3 — `attest-control-plane` (REST API)

An Axum 0.8 HTTP server that exposes the RisingWave views as a REST API. The control-plane also applies the RisingWave DDL on startup so there are no manual migration steps.

```
attest-control-plane :8080
  ├── GET  /healthz
  ├── GET  /v1/events/recent?id=<uuid>   → recent_events view
  └── GET  /v1/baselines/user/:name      → entity_baselines view
```

**E2E acceptance gate:** Post a `ConsoleLogin` event → poll `recent_events` (≤ 5 s) → poll `entity_baselines` for the user's region (≤ 30 s). Run with `make e2e-phase1`.

---

### Phase 2 — Iceberg Warm Tier + Web App Backend Connection

**The problem it solves:** Redpanda is a streaming buffer (short retention). RisingWave is great for real-time queries of recent events but not for cheap, long-term storage of millions of events. You need a "warm tier" — somewhere events land after they're a few minutes old, queryable for analytics and historical investigation. Phase 2 also wires the Next.js workbench UI to the live backend so the SOC console shows real data instead of mock fixtures.

#### Pillar 1 — `attest-storage-iceberg` (Kafka → Parquet → MinIO)

A new Rust binary that consumes the same `cloudtrail` Redpanda topic and persists events as **Parquet files in MinIO** (S3-compatible local object store), date-partitioned in Hive format for efficient time-range pruning.

Two flush triggers race inside a `tokio::select!` loop — whichever fires first wins:
- **Batch full** — every 1,000 events
- **Timer** — every 30 seconds (catches low-traffic periods)

```
Redpanda (topic: cloudtrail)
      ↓  IcebergConsumer (rdkafka StreamConsumer)
      ↓  buffer events in memory
      ↓  flush on: batch >= 1000  OR  30s elapsed
      ↓  FlatEvent[] → Arrow RecordBatch → Parquet bytes
MinIO :9000  (bucket: attest-warm)
  └── cloudtrail/year=YYYY/month=MM/day=DD/<uuid>.parquet
```

#### Pillar 2 — `POST /v1/warm/query` (ClickHouse as Serverless Query Engine)

ClickHouse has a built-in `s3()` table function that reads Parquet files directly from MinIO — no ETL, no table definitions, no schema migrations. Phase 2 exposes this through a new control-plane endpoint.

The flow for a warm query:
1. Client sends `{"sql": "SELECT count(*) FROM s3(...)"}`
2. Handler validates the SQL starts with `SELECT` (safety gate — no writes allowed)
3. Appends `FORMAT JSONCompact` and POSTs to `http://clickhouse:8123/`
4. ClickHouse reads Parquet from MinIO on demand and returns results
5. Control-plane parses `{"data": [[row1]...]}` and returns `{"rows": [...], "row_count": N}`

```
POST /v1/warm/query  { "sql": "SELECT ..." }
      ↓  attest-control-plane :8080
      ↓  safety check (SELECT only)
      ↓  reqwest POST → ClickHouse HTTP API :8123
      ↓  s3() table function reads Parquet files
MinIO :9000  (attest-warm bucket)
```

#### Pillar 3 — Next.js Workbench Wired to Live Backend

Three Next.js proxy API routes were added so the browser never talks directly to the Rust services (avoids CORS, keeps backend URLs server-side):

| Proxy Route | Forwards to |
|---|---|
| `GET /api/events` | `control-plane /v1/events/recent` |
| `GET /api/baselines/user/[name]` | `control-plane /v1/baselines/user/:name` |
| `POST /api/warm/query` | `control-plane /v1/warm/query` |

**Queue page** (`/workbench/queue`) — Server Component that fetches `GET /v1/events/recent` with ISR revalidation every 5 seconds, maps OCSF events to the `Alert` shape via `ocsf-to-alert.ts`, and falls back to mock data silently if the backend is offline. A status badge shows **"Live — control-plane connected"** or **"Mock data — control-plane offline"**.

**Case detail page** (`/workbench/cases/[id]`) — Fetches the specific event by ID and the actor's 30-day baseline in parallel. Live data is merged on top of the mock case shell; the baseline regions appear as an evidence card in the investigation panel.

**E2E acceptance gate:** Seed 10,000 events via the collector → wait ≤ 90 s for Iceberg flush → assert `SELECT count(*)` via warm query returns ≥ 10,000 → assert a `GROUP BY cloud_region` aggregate completes in ≤ 30 s. Run with `make e2e-phase2`.

---

### Phase 3 — HELIQL DSL v0 + Stream Detection

**The problem it solves:** Detection logic written in raw SQL or Kafka consumer code is hard to audit, impossible to backtest, and tied to a single engine. A portable detection DSL lets security engineers write rules once and have them run on the live stream (RisingWave), on historical data (ClickHouse over Iceberg), or federated to external engines — without changing the rule.

#### Pillar 1 — `attest-heliql` (Parser + Compiler)

A Rust library that defines the HELIQL v0 language. It has three layers:

- **Grammar** (`grammar.pest`) — a `pest` PEG grammar covering comparisons, IN/NOT IN with baseline references, unique-count temporal windows, AND/OR boolean expressions, severity, MITRE tags, and runtime targets.
- **AST** (`ast.rs`) — typed Rust structs representing a parsed `Detection`, `Condition`, `ConditionAtom`, `Duration`, and `BaselineRef`.
- **Compiler** (`compiler.rs`) — walks the AST and emits a `CREATE MATERIALIZED VIEW IF NOT EXISTS det_<id>` RisingWave DDL. Baseline references compile to correlated subqueries against the `entity_baselines` view built in Phase 1; temporal windows compile to `count(DISTINCT ...)` subqueries with `INTERVAL` predicates.

```
detection: aws_console_login_from_anomalous_geolocation
where:
  - event.auth_status = "Success"
condition:
  - event.cloud_region NOT IN baseline(identity.user, 90d)
severity: medium
mitre: [T1078.004]
runtime: stream | batch
```
↓ compiles to →
```sql
CREATE MATERIALIZED VIEW IF NOT EXISTS det_aws_console_login_from_anomalous_geolocation AS
SELECT 'aws_console_login_from_anomalous_geolocation' AS detection_id,
       e.event_id, e.actor_user_name, e.cloud_region, 'medium' AS severity, NOW() AS fired_at
FROM cloudtrail_events e
WHERE auth_status = 'Success'
  AND NOT EXISTS (SELECT 1 FROM entity_baselines eb WHERE ...);
```

The Sigma compatibility module (`sigma.rs`) converts Sigma YAML rules into the same HELIQL AST so existing Sigma content can be imported directly.

#### Pillar 2 — `attest-detection-runtime` (Deploy + Poll + Emit)

A Rust binary with a three-phase lifecycle:

1. **Load** — scans the `RULES_DIR` (`/rules` by default, populated by the `detections/` folder mounted as a Docker volume) and parses every `*.heliql` file.
2. **Deploy** — compiles each rule and executes the DDL against RisingWave. Rules that fail to compile are logged and skipped.
3. **Poll loop** — every 2 seconds, `SELECT` the latest rows from each `det_*` materialized view and produce them as JSON to the `alerts` Redpanda topic.

```
*.heliql files  (mounted from detections/)
      ↓  loader: parse all rules
      ↓  deployer: CREATE MATERIALIZED VIEW det_* in RisingWave
RisingWave :4566  (continuously evaluates detection views against stream)
      ↓  poller: SELECT fired rows every 2s
      ↓  produce JSON alert to Kafka
Redpanda :9092  (topic: alerts)
```

#### Pillar 3 — 10 Bundled MVP Detection Rules

Ten HELIQL rules in `detections/` covering the MITRE techniques most common in CloudTrail/Okta/M365 sources:

| Rule file | Technique | Severity |
|---|---|---|
| `aws_login_anomalous_geo.heliql` | T1078.004 | medium |
| `aws_root_account_use.heliql` | T1078 | critical |
| `okta_brute_force.heliql` | T1110 | high |
| `okta_mfa_bypass.heliql` | T1556.006 | high |
| `m365_mass_external_sharing.heliql` | T1567.002 | high |
| `m365_inbox_auto_forward.heliql` | T1564.008 | high |
| `aws_s3_bucket_made_public.heliql` | T1567.002 | high |
| `aws_iam_excessive_privilege.heliql` | T1078.004 | high |
| `aws_cloudtrail_logging_disabled.heliql` | T1562.001 | critical |
| `aws_new_iam_user_then_keys.heliql` | T1136 + T1098 | high |

**E2E acceptance gate:** Seed alice with 30 US-region logins (baseline) → inject a login from `ap-southeast-1` → assert an alert appears on the `alerts` Redpanda topic with `detection_id = aws_console_login_from_anomalous_geolocation` within 10 s. Run with `make e2e-phase3`.

---

## Crates

| Crate | Phase | Purpose |
|---|---|---|
| `crates/attest-common` | 1 | OCSF 1.3 types: `OcsfEvent`, `AuthenticationEvent` (class 3002), `CloudActivityEvent` (class 6003) |
| `crates/attest-collector` | 1 | Edge collector — CloudTrail JSON → OCSF normalizer → Redpanda + Axum HTTP ingest |
| `crates/attest-control-plane` | 1 + 2 | Axum 0.8 REST API — RisingWave hot tier + ClickHouse warm tier |
| `crates/attest-storage-iceberg` | 2 | Kafka consumer → Arrow/Parquet batch writer → MinIO via `object_store` |
| `crates/attest-heliql` | 3 | HELIQL DSL — pest grammar, AST, RisingWave SQL compiler, Sigma importer |
| `crates/attest-detection-runtime` | 3 | Loads `.heliql` rules, deploys as RisingWave views, polls and emits alerts to Redpanda |

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

Starts Redpanda (`:9092`, `:19092`), RisingWave (`:4566`), Postgres (`:5432`), MinIO (`:9000`, console `:9001`), and ClickHouse (`:8123`) in Docker. The `minio-init` one-shot container creates the `attest-warm` bucket automatically and exits with code 0.

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
# Posts a ConsoleLogin event → polls recent_events (≤ 5 s) → polls entity_baselines for user region (≤ 30 s).

# Phase 2 — Iceberg warm tier
make e2e-phase2
# Seeds 10 000 events → waits for Iceberg flush to MinIO (≤ 90 s) → asserts ClickHouse count + GROUP BY aggregate (≤ 30 s).

# Phase 3 — HELIQL detection engine
make e2e-phase3
# Seeds alice's US-region baseline → injects ap-southeast-1 login → asserts alert on `alerts` Kafka topic within 10 s.
# Also tests: StopLogging event fires a critical alert for aws_cloudtrail_logging_disabled.
```

All targets require `make dev-up-platform` to be running first.

---

## Project Structure

```
Attest/
├── crates/
│   ├── attest-common/            # OCSF 1.3 types (Phase 1)
│   ├── attest-collector/         # CloudTrail → Redpanda edge collector (Phase 1)
│   ├── attest-control-plane/     # Axum 0.8 REST API — hot + warm tier (Phase 1 + 2)
│   ├── attest-storage-iceberg/   # Kafka → Parquet → MinIO writer (Phase 2)
│   ├── attest-heliql/            # HELIQL DSL parser + RisingWave compiler (Phase 3)
│   └── attest-detection-runtime/ # Detection deploy + poll + emit to alerts topic (Phase 3)
├── detections/                   # 10 bundled HELIQL detection rules (Phase 3)
├── apps/
│   └── workbench/              # Next.js 15 marketing site + SOC workbench UI
├── infra/
│   ├── clickhouse/             # ClickHouse config (listen + S3/MinIO access)
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
| `make e2e-phase3` | Run Phase 3 E2E test (requires `ATTEST_E2E=1` + running stack) |
| `make fmt` | `cargo fmt --all` |
| `make lint` | `cargo clippy` + `bun run lint` |
| `make smoke` | Quick smoke check — cargo test + bun test + pytest |
| `make dev-up-llm` | Start core infra + llama.cpp (requires Qwen GGUF volume-mounted) |

---

## Railway Deployment

### Service Map

| Railway service | Image / Builder | Internal hostname |
|---|---|---|
| `redpanda` | `confluentinc/cp-kafka:7.9.0` | `redpanda.railway.internal:9092` |
| `risingwave` | `risingwavelabs/risingwave:latest` | `risingwave.railway.internal:4566` |
| `clickhouse` | `clickhouse/clickhouse-server:latest` | `clickhouse.railway.internal:8123` |
| `minio` | `minio/minio:latest` | `minio.railway.internal:9000` |
| `collector` | Dockerfile `infra/docker/collector.Dockerfile` | — |
| `control-plane` | Dockerfile `infra/docker/control-plane.Dockerfile` | `control-plane-production-b6e3.up.railway.app` |
| `storage-iceberg` | Dockerfile `infra/docker/storage-iceberg.Dockerfile` | — |
| `detection-runtime` | Dockerfile `infra/docker/detection-runtime.Dockerfile` | — |
| `workbench` | Nixpacks (`nixpacks.toml`) | `workbench-production-6e86.up.railway.app` |

### First-time setup

```sh
railway login
make railway-infra        # create the 4 Docker-image infrastructure services
make railway-infra-config # set start commands + env vars for infra services
make railway-setup        # create the 5 app services and configure builders
make railway-deploy       # upload source + trigger first build for all app services
make railway-domain       # generate public HTTPS domains for control-plane + workbench
```

After `make railway-domain`, copy the two domains into workbench's Railway env vars:

```sh
railway variable set --service workbench \
  CONTROL_PLANE_URL=https://<control-plane-domain> \
  NEXT_PUBLIC_APP_URL=https://<workbench-domain> \
  NEXT_PUBLIC_CP_WS_URL=wss://<control-plane-domain>
```

### Subsequent deploys

```sh
# Re-upload source + rebuild (needed when code changes)
make railway-deploy

# Or just restart the last build (when only env vars / config changed)
make railway-redeploy
```

### Monitoring

```sh
make railway-status            # overview of all services
make railway-logs              # tail all app services in parallel
make railway-logs-collector    # tail a single service
make railway-build-logs-workbench  # tail build output
```

---

### Hard-won Railway lessons (do not repeat)

#### 1. Redpanda cannot run on Railway
Redpanda's Seastar I/O engine requires `perf_event_open` syscall and Linux AIO — both blocked in Railway's container sandbox. Redpanda starts, passes the health check, then crashes within ~10 seconds. **Use `confluentinc/cp-kafka:7.9.0` (KRaft mode) instead.** It uses standard Java I/O and runs fine. The service is still named `redpanda` so no app env vars need updating.

KRaft requires a `CLUSTER_ID` (22-char base64 UUID). Generate one with:
```sh
python3 -c "import base64, uuid; print(base64.urlsafe_b64encode(uuid.uuid4().bytes).decode().rstrip('='))"
```
Required env vars for single-node KRaft:
```
CLUSTER_ID=<generated>
KAFKA_NODE_ID=1
KAFKA_PROCESS_ROLES=broker,controller
KAFKA_CONTROLLER_QUORUM_VOTERS=1@localhost:9093
KAFKA_LISTENERS=PLAINTEXT://:9092,CONTROLLER://:9093
KAFKA_ADVERTISED_LISTENERS=PLAINTEXT://redpanda.railway.internal:9092
KAFKA_CONTROLLER_LISTENER_NAMES=CONTROLLER
KAFKA_LISTENER_SECURITY_PROTOCOL_MAP=CONTROLLER:PLAINTEXT,PLAINTEXT:PLAINTEXT
KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR=1
KAFKA_TRANSACTION_STATE_LOG_REPLICATION_FACTOR=1
KAFKA_TRANSACTION_STATE_LOG_MIN_ISR=1
KAFKA_AUTO_CREATE_TOPICS_ENABLE=true
KAFKA_LOG_DIRS=/var/lib/kafka/data
```

#### 2. `railway.json` at repo root applies to ALL services
Every service that has its root directory set to `/` reads the same `railway.json`. If that file contains a `deploy.startCommand` (e.g. `"cd apps/workbench && bun run start"`), **every service** will attempt to run it — Rust containers with minimal images will crash with `executable 'cd' not found`. Keep `railway.json` free of any `startCommand`; set start commands per-service via the Railway dashboard or `railway environment edit`.

#### 3. `railway environment edit` is not interactive in CI — use JSON stdin
The `--service-config <name> <path> <value>` flag works interactively (TTY prompt) but blocks in scripts. The reliable non-interactive method is to pipe a JSON patch to stdin:

```sh
python3 -c "import json; print(json.dumps({'services': {'<UUID>': {'deploy': {'startCommand': 'attest-collector serve'}}}}))" \
  | railway environment edit --message "fix start command"
```

**Critical:** the JSON keys must be **service UUIDs**, not service names. Get UUIDs with:
```sh
railway environment config --json | python3 -c "
import json,sys; data=json.load(sys.stdin)
for uid,cfg in data['services'].items():
    print(uid, cfg.get('build',{}).get('dockerfilePath',''), cfg.get('source',{}).get('image',''))
"
```

#### 4. `${{ServiceName.VAR}}` interpolation is case-sensitive
Railway variable references like `${{Risingwave.RAILWAY_PRIVATE_DOMAIN}}` only work if the service name casing matches exactly. Since all services here are lowercase (`risingwave`, `clickhouse`, `minio`, `redpanda`), use **literal hostnames** instead of interpolation:
- `risingwave.railway.internal`
- `clickhouse.railway.internal`
- `minio.railway.internal`
- `redpanda.railway.internal`

#### 5. `railway service redeploy` re-runs the last deployment snapshot
For source-built services, `railway service redeploy` re-runs the old build artifact with its original `railway.json` baked in. Config changes in `railway.json` are **not** picked up. You must run `railway up --service <name>` to push a new build that uses the current file. For Docker-image services, config changes (start command, env vars) ARE picked up by redeploy.

#### 6. MinIO requires a persistent volume and the `minio` binary prefix
MinIO's Docker entrypoint does not forward `CMD` arguments. Set the start command to `minio server /data --console-address :9001` (with the `minio` binary prefix). Without a volume, MinIO formats a new pool on every restart and exits cleanly — add a Railway persistent volume mounted at `/data`:
```sh
railway volume -s <minio-uuid> add -m /data
```

#### 7. Kafka topic pre-creation for RisingWave sources
RisingWave's `CREATE TABLE ... WITH (connector='kafka')` fetches Kafka metadata but does **not** trigger Kafka's `auto.create.topics.enable`. The topic must already exist before the DDL runs. The `attest-control-plane` now creates the `cloudtrail` topic via the rdkafka admin client on startup (see `ensure_kafka_topic` in `main.rs`). Do not add `properties.allow.auto.create.topics = 'true'` to the RisingWave WITH clause — it is not a valid connector property and will cause `CREATE TABLE` to fail with "Unknown fields".

#### 8. Workbench Node.js version
Nixpacks `[variables] NODE_VERSION = "20"` in `nixpacks.toml` sets an environment variable but does **not** pin the Node.js version used during the build phase. The `.node-version` file at the repo root (containing `20`) is the correct signal that Nixpacks respects.

#### 9. `bitnami/kafka` has no `latest` tag
`bitnami/kafka:latest` does not exist — use `bitnami/kafka:3.9` or a specific version. Alternatively, use `confluentinc/cp-kafka:7.9.0` (which is what this project uses) or `apache/kafka:latest` (official image, does have `latest`).

#### 10. Detection rules baked into the Docker image
Railway does not support local volume mounts from the host. Detection rules (`.heliql` files) are copied into the `detection-runtime` image at build time via `COPY detections/ /rules/` in `infra/docker/detection-runtime.Dockerfile`. The `RULES_DIR=/rules` env var is set in the Dockerfile. This is intentional and correct for Railway deployments.
