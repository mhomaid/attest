# Attest — phase chronicle

This is the detailed, phase-by-phase build log (what shipped in Phases 1–8, page-by-page
workbench notes, Railway lessons). It is **not** the project README.

For what is implemented today, start with the Status table in the [root README](../README.md).
A few details below are historical (for example the warm-tier SQL gate is now a real
tokenizer, not a `starts_with("select")` check; warm storage is Parquet, not an Iceberg catalog).

---

# Attest

Attest is a streaming-first, agent-aware security operations platform built for cloud-native security teams. Events flow from cloud sources (CloudTrail, Okta, Entra ID) through an OCSF normalizer into Redpanda, are continuously aggregated by RisingWave materialized views, persisted as Parquet files in MinIO via Apache Iceberg, and queried at scale by ClickHouse — all exposed through a REST control-plane and a live Next.js SOC workbench.

The agent layer runs on top of the data tier. A Hybrid Triager routes every alert through an XGBoost classifier (P99 under 5 ms per inference, asserted in CI) or escalates structurally novel cases to an LLM. When the verdict is `needs_investigation`, an **Investigator** agent runs a tool loop through the MCP gateway (including optional warm-tier SQL). Every agent decision produces a cryptographically signed `AttestationEnvelope` — not a black box.

## Documentation

| File | What it covers |
|---|---|
| `docs/README.md` | Product and architecture overview |
| `docs/01_PRD.md` | Product requirements document |
| `docs/02_Architecture.md` | System architecture (six-plane model, principles, components) |
| `docs/03_Architecture_Diagrams.md` | All Mermaid diagrams — C4, data flow, sequences, state machines |
| `docs/07_Stack_Revised.md` | Canonical tech stack |
| `docs/15_Streaming_Engine_Decision.md` | ADR: Arroyo vs. Flink vs. RisingWave |
| `docs/10_Build_Order.md` | Phase-by-phase implementation sequence |
| `docs/11_Repo_Structure.md` | Repository layout |
| `docs/12_Workbench.md` | Workbench UX spec (SOC analyst flows, components, Phase 8+) |

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

### Phase 4a — Hybrid Triager Agent (Classifier Path)

**The problem it solves:** SIEM alerts arrive faster than humans can triage them. A rule-based classifier handles the 80 % of high-confidence, in-distribution alerts instantly. Low-confidence or structurally novel alerts are flagged for LLM escalation (Phase 4b). Every decision is cryptographically signed and logged — not a black box.

#### Pillar 1 — ML Pipeline (`ml/triager/`)

A Python pipeline (managed with `uv`) that trains the classifier artifacts the Rust runtime loads at startup:

| Script | What it produces |
|---|---|
| `train.py` | XGBoost classifier → `model.onnx` + `shap_background.npy` + SHA-256 hashes |
| `novelty.py` | Mahalanobis covariance parameters → `novelty_mean.npy` + `novelty_inv_cov.npy` + `novelty_threshold.txt` |
| `calibrate.py` | Isotonic regression calibration → `calibration_models.pkl`; also serves a FastAPI HTTP sidecar on `:5001` |

```
ml/triager/golden_cases.json  (210 labelled alerts, 10 OOD)
      ↓  train.py
ml/triager/artifacts/model.onnx          (XGBoost, 8-feature binary classifier)
ml/triager/artifacts/novelty_*.npy       (Mahalanobis covariance)
ml/triager/artifacts/calibration_models.pkl  (isotonic regression per case class)
```

Run the full pipeline with:
```sh
make train-classifier
```

#### Pillar 2 — `attest-onnx-runtime` (In-Process Inference)

Pure-Rust inference using `tract-onnx` — no Python or `ort` at runtime:

- **`OnnxClassifier`** — loads `model.onnx`, runs forward pass, approximates per-feature SHAP contributions
- **`NoveltyDetector`** — loads Mahalanobis parameters, computes distance score, classifies alerts as in-distribution or OOD

#### Pillar 3 — `attest-feature-extractor`

Converts an `OcsfEvent` into the 8-dimensional `AlertFeatures` vector the classifier expects:

| Feature | Source |
|---|---|
| `severity_score` | OCSF severity enum → `[0.0, 1.0]` |
| `source_class_id` | OCSF class ID (mapped) |
| `entity_reputation_score` | stub → threat intel enrichment in Phase 7 |
| `baseline_deviation` | stub → RisingWave baselines in Phase 7 |
| `threat_intel_hit_count` | stub |
| `hour_of_day` | event timestamp |
| `asset_criticality` | stub |
| `prior_disposition_ratio` | stub |

#### Pillar 4 — `attest-attestation` (Signed Evidence Envelopes)

Every agent decision produces a tamper-evident `AttestationEnvelope` signed with Ed25519 (any field change fails verification — see `tampered_envelope_fails_verification`). Deterministic replay of classifier decisions from the envelope is planned:

```
AttestationEnvelope {
  agent_action_id,    // unique per decision
  case_id,
  execution_path,     // Classifier | Hybrid | Llm
  verdict,            // true_positive | benign | needs_investigation | escalated_stub
  evidence: ClassifierEvidence | HybridEvidence | LlmEvidence,
  timing,             // wall-clock start/end
  signature,          // Ed25519 over canonical JSON, hex-encoded
}
```

Envelopes are appended to `attestations.ndjson` (newline-delimited JSON). The verifying key is published at `GET /agent` so signatures can be verified offline.

#### Pillar 5 — `attest-policy-engine` (Deterministic Auth)

Hard-coded Rust policies that authorize every tool call before it executes. No network round-trip, no database lookup.

```
authorize(role, tool_id, PolicyContext) → Allow | Deny | Escalate
```

Roles: `Triager`, `Investigator`, `Responder`, `Auditor`. Read-only tools (`query_hot_tier`, `lookup_threat_intel`) are always allowed. Destructive actions (`idp_revoke_session`, `isolate_host`) require high confidence, non-protected targets, and pass blast-radius checks.

#### Pillar 6 — `attest-mcp-gateway` (Tool Call Intercept)

A standalone Axum service that intercepts every tool call, enforces policy, logs the interaction, and forwards to tool implementations. Built now as a foundation for the LLM escalation path in Phase 4b.

```
POST /invoke  { agent_role, tool_id, args, ... }
      ↓  argument hash (SHA-256)
      ↓  authorize() via attest-policy-engine
      ↓  dispatch to tool stub (or external backend)
      ↓  log ToolCallRecord
      → { result, allowed, latency_ms, args_hash }

GET /tools    → list of registered ToolDescriptors
GET /healthz  → `ok` (same liveness pattern as other HTTP services)
```

#### Pillar 7 — `attest-orchestrator` (Hybrid Triage Loop)

The agent runtime. On each `POST /triage` it runs the full Hybrid path:

```
POST /triage  { alert_json }
      ↓  FeatureExtractor → AlertFeatures (8 f64 values)
      ↓  OnnxClassifier → raw_score + shap_values
      ↓  CalibrationClient → calibrated_score  (HTTP → Python sidecar)
      ↓  NoveltyDetector → novelty_score
      ↓  routing decision:
         calibrated ≥ threshold AND in-distribution → Classifier path
         otherwise                                  → EscalatedStub (Phase 4b wires LLM here)
      ↓  build AttestationEnvelope + Ed25519 sign
      ↓  append to attestations.ndjson
      → TriageVerdict { verdict, confidence, action_id, latency_ms, ... }

GET /agent  → agent definition + Ed25519 verifying key for offline signature verification
GET /healthz
```

**E2E acceptance gate (all passing):**
- Known brute-force pattern → `classifier` path, calibrated confidence ≥ 0.5, latency < 500 ms
- OOD structurally novel alert → `hybrid` path, `escalated_stub` verdict, novelty score > 0
- Classifier path P99 latency over HTTP < 200 ms, end to end including the calibration sidecar (measured: **28 ms**)
- Classifier inference + attribution in-process, release build: P99 < 5 ms over 1,000 runs (measured: **0.45 ms**; `classifier_predict_p99_under_budget`)

Run with `make e2e-phase4a` (automatically starts services, runs tests, cleans up).

---

### Phase 4b — Load Lab + Simulate Lab (Observability & Testing Tools)

**The problem they solve:** It's not enough to build a streaming detection platform — you need to *see* it work at scale and verify every component independently. Phase 4b adds two complementary tools:

#### Load Lab (`/workbench/load`)

Exercises the **streaming rules path** (Kafka → RisingWave → Detection Runtime → alerts) at configurable throughput. Real-time metrics stream via WebSocket at 1 Hz.

```
Load Gen ──► Kafka cloudtrail ──► RisingWave (SQL rules) ──► Detection Runtime ──► Kafka alerts
  :9100                                                                                    │
                                                            also N% ──► Orchestrator :4300 │
                                                            (sampled triage)               │
Control Plane :8080  ◄── 1 Hz MetricsSnapshot (WS) ──────────────────────────────────────┘
  │  events_per_sec     — Kafka cloudtrail HWM delta
  │  consumer_lag       — RisingWave consumer group offset gap
  │  detections_per_sec — Kafka alerts HWM delta (rules fired)
  │  storage_rows_per_sec — ClickHouse warm-tier ingestion rate
  └  triage_p95_ms      — ML orchestrator latency (only when sampled_triage_pct > 0)
```

**Presets:**

| Preset | Rate | Duration | Triage sampling |
|---|---|---|---|
| Smoke | 1k/sec | 30s | 5% |
| Sustained | 10k/sec | 60s | 5% |
| Burst | 100k/sec | 60s | 1% |
| 1M Challenge | 100k/sec | 120s | off |

The **Sampled Triage** slider (0–20%) controls what percentage of load events are also sent to the ML orchestrator concurrently. This gives you real triage p95 latency under load without overwhelming the orchestrator. Set it to 0 for maximum streaming throughput.

#### Simulate Lab (`/workbench/simulate`)

Exercises the **ML hot-path** (Collector → Orchestrator ONNX → Calibration → Attestation) with a single event and shows every stage's result. Also includes **Batch Mode** (50 concurrent triage calls) to measure ML latency distribution under concurrency.

```
Single event ──► POST /api/simulate ──► Collector :4000 ──► Orchestrator :4300
                                                                     │
                                              FeatureExtractor → AlertFeatures
                                              OnnxClassifier → raw_score + SHAP
                                              CalibrationClient → calibrated_score
                                              NoveltyDetector → novelty_score
                                              AttestationEnvelope (Ed25519 signed)
                                                     │
                                         TriageVerdict → UI (verdict, confidence, SHAP panel)
```

**Batch Mode** fires 50 concurrent POST /triage calls and returns min/p50/p95/p99/max latency — tells you exactly where the orchestrator's latency envelope sits under real concurrency.

---

### Phase 5 — Hallucination guardrails

**The problem it solves:** LLM triage must not fabricate evidence. Guardrails enforce retrieval-first answers, citation realism (orphan citations → `needs_investigation`), and optional cross-agent review at high calibrated confidence.

- Implemented in `crates/attest-orchestrator/src/guardrails.rs`, wired into the hybrid / LLM triage path.
- Toggle with `ATTEST_GUARDRAILS` (`on` / `off`); retries capped by `GUARDRAIL_MAX_RETRIES`.

**E2E:** `make e2e-phase5` (scripted LLM + WireMock; see `tests/e2e-tests/tests/phase5_guardrails.rs`).

---

### Phase 6 — Shadow check + triager auto-close

**The problem it solves:** High-confidence benign verdicts should be able to auto-close a case only when deterministic policy allows it (tenant automation, action class allowlist, do-not-touch principals, severity floor).

- `crates/attest-orchestrator/src/shadow_check.rs` + `auto_close.rs`.
- Environment knobs: `AUTO_CLOSE_THRESHOLD`, `AUTO_CLOSE_ACTION_CLASSES`, `DO_NOT_TOUCH_LIST`, `TENANT_ALLOWS_AUTOMATION`, etc. (see `env.example`).

**E2E:** `make e2e-phase6` (`tests/e2e-tests/tests/phase6_auto_close.rs`).

---

### Phase 7 — Investigator agent + warm-tier MCP + attestation trace

**The problem it solves:** When the triager returns `needs_investigation`, a dedicated **Investigator** LLM runs a tool loop (MCP gateway → control-plane warm tier and stubs), produces a second signed envelope, and analysts need an **inspectable trace** per case.

#### Components

| Piece | Role |
|---|---|
| `attest-orchestrator` | On `needs_investigation`, runs `run_investigator_llm_loop`; dispatches tools via `attest-mcp-client` → MCP gateway |
| `agents/investigator/system_prompt_v1.md` | Investigator system prompt |
| `attest-mcp-gateway` | `POST /invoke`, `GET /tools`, `GET /healthz`; `query_warm_tier` wraps control-plane `/v1/warm/query` with rate limits (`WarmTierLimiter`) |
| `attest-inference-router` | OpenAI-compat client: by default encodes tool **results** as `role: "user"` (Unsloth / strict servers reject `role: "tool"`). Set `ATTEST_OPENAI_NATIVE_TOOL_MESSAGES=1` for native tool messages |
| `apps/workbench-api` | `GET /v1/cases/{case_id}/trace` — reads `ATTEST_LOG_PATH` NDJSON, returns `{ case_id, steps[] }` for the workbench **Attestation trace** panel |
| `apps/workbench` | `GET /api/cases/[caseId]/trace` proxies to workbench-api (server-side env) |

#### Attestation log path (critical for trace)

Both **orchestrator** and **workbench-api** must use the **same** `ATTEST_LOG_PATH` (see `env.example`). The Docker Compose orchestrator writes to `/data/attestations.ndjson` on the `attestation-log` volume; a workbench-api process on the host will **not** see that unless you bind-mount or run the orchestrator **natively** with the same path.

**Offline E2E (no live LLM):** `make e2e-phase7` — `investigator_loop` integration test with scripted chat client + WireMock MCP.

**Live E2E (real stack):** With Unsloth (or other OpenAI-compat server), MCP gateway, and control-plane up:

1. Terminal A: `make run-orchestrator`  
2. Terminal B: `make run-workbench-api`  
3. `ATTEST_E2E=1 ATTEST_PHASE7_LIVE=1 make e2e-phase7-live`  

Optional: `ATTEST_PHASE7_LIVE_STRICT=1`, `PHASE7_REQUIRE_WARM_QUERY=1`. Full notes: `scripts/e2e/README.md`.

---

## Workbench UI — Page Guide

The workbench is a Next.js 16 App Router application at `apps/workbench`. Every page is a React Server Component that fetches live data from the backend on each request. No mock data is used — if the backend is offline, pages show explicit offline states. Client components (Load Lab, Simulate Lab) use Zustand stores for cross-navigation state.

### Page Map

| Route | Live backend calls? | Purpose |
|---|---|---|
| `/workbench/queue` | `control-plane /v1/detections/fired` + `arroyo /api/v1/pipelines` + WS | Real-time alert queue |
| `/workbench/cases` | `control-plane /v1/detections/fired` (via `/api/detections`) | All triaged cases |
| `/workbench/cases/[id]` | event lookup + baseline + triage + `/api/cases/[id]/trace` → workbench-api | Deep case investigation + attestation trace |
| `/workbench/detections` | `control-plane /v1/detections/fired` (last-fired timestamps) | Detection rule catalogue |
| `/workbench/simulate` | `/api/simulate` → collector + orchestrator | ML hot-path testing |
| `/workbench/load` | `/api/load/start` + `control-plane WS /v1/metrics/stream` | Streaming throughput testing |
| `/workbench/agents` | `orchestrator /metrics` | Agent roster + live latency |
| `/workbench/hunt` | Partial — warm-tier wired in UI; stream/save still shells | Threat hunting query editor |
| `/workbench/settings` | None — static | Integration & agent configuration status |
| `/workbench/admin` | 4 health checks + Arroyo pipelines API | Platform health + API docs links |

---

### Queue (`/workbench/queue`) — default landing page

Fetches all fired detections from `control-plane /v1/detections/fired` and the list of running Arroyo pipelines. A status badge shows one of three states:

- **`Offline — control-plane unreachable`** — backend is down
- **`Connected — no detections yet`** — backend is up but no rules have fired
- **`Live — HELIQL detections connected`** — at least one detection is present

Below the header, `AlertQueueLive` is a client component that connects to `control-plane /v1/ws/alerts` via WebSocket and pushes new detections in real time. Arroyo pipeline chips (clickable, link to Arroyo UI) show how many streaming pipelines are currently running.

**What to watch during a test:** Run a simulation or load test → alerts appear in the queue within 2–10 seconds → pipeline chips turn green.

---

### Cases (`/workbench/cases`)

Lists all fired alerts bucketed into **Open** and **Auto-Closed**. Calls `/api/detections` (a Next.js proxy to `control-plane /v1/detections/fired`) and maps each `FiredDetection` to an `Alert` via `detection-to-alert.ts`.

Each row shows: severity badge, event title, actor username, region, data source, confidence %, and ML execution path. Clicking a row navigates to the case detail page using the event UUID as the route parameter.

**When empty:** "No cases in this state. Run a load test or send events to generate detections."

---

### Case Detail (`/workbench/cases/[id]`)

The most data-intensive page. Three backend calls run in parallel on every load:

```
1. control-plane  GET /v1/events/recent?id={id}        → raw OCSF event from RisingWave hot tier
2. control-plane  GET /v1/baselines/user/{username}    → 30-day behavioural baseline (regions, event count)
3. orchestrator   POST /triage { alert: <event> }      → live ML verdict + SHAP feature values
```

The page builds a `CaseRecord` entirely from live data and renders it in `CaseWorkbench`:

- **Timeline** — each step the event traversed: Collector (OCSF normalization) → RisingWave (baseline fetch) → Orchestrator (ML verdict) → Attestation (Ed25519 signing)
- **Attestation trace** — `AttestationTracePanel` loads `GET /api/cases/{case_id}/trace` (NDJSON steps: triage, investigator, auto-close, etc.); requires `WORKBENCH_API_URL` and a shared `ATTEST_LOG_PATH` with the orchestrator
- **Evidence panel** — Event ID, API operation, region, baseline regions seen in 30 days, triage action ID, novelty score
- **SHAP feature impact bars** — top 8 features that drove the ML classifier's decision

If the event ID is not found in the hot tier (expired or backend offline): "Case not found — event may have expired from the hot tier."

---

### Detections (`/workbench/detections`)

Displays the **10 bundled HELIQL detection rules** compiled into RisingWave as materialized views. The rule metadata (title, description, MITRE ATT&CK ID) is static in the UI. The `Last fired` column is live — it queries `/api/detections` and finds the most-recent `fired_at` timestamp per `detection_id`.

Rows with a recent match glow green. The header shows "N rules fired" based on how many have at least one match in the backend.

| Rule | Technique | Severity |
|---|---|---|
| Console Login from Anomalous Region | T1078.004 | medium |
| CloudTrail Logging Disabled | T1562.001 | critical |
| AWS Root Account Used | T1078 | critical |
| Excessive IAM Privilege Granted | T1098 | high |
| New IAM User + Access Keys Sequence | T1136.003 | high |
| S3 Bucket Made Public | T1530 | high |
| Okta Brute-Force Authentication | T1110 | high |
| Okta MFA Bypass Attempt | T1556 | high |
| M365 Mass External Sharing | T1567 | high |
| M365 Inbox Auto-Forward Rule | T1114.003 | critical |

---

### Simulate Lab (`/workbench/simulate`)

A client component. Tests the **ML hot path** with a single injected event and shows per-stage results in real time.

**Left column** — 5 named attack scenarios (each with MITRE ID, severity, description).

**Centre column** — Editable parameters: actor username, source IP, region, severity score slider. "Run Simulation" and "Batch (50× concurrent)" buttons.

**Right column — Hot Path** shows 4 animated stage rows:

| Stage | Service | What it proves |
|---|---|---|
| Collector | `attest-collector :4000` | CloudTrail → OCSF normalized + published to Kafka |
| Orchestrator | `attest-orchestrator :4300` | Hybrid triage loop (XGBoost ONNX + Mahalanobis novelty) |
| Calibration | `calibration-sidecar :5001` | Isotonic regression — raw score → calibrated probability |
| Attestation | `attest-attestation` crate | Ed25519 signed envelope appended to `attestations.ndjson` |

After the hot path completes, a **verdict card** shows benign/suspicious/malicious, calibrated confidence bar, and novelty score. The **SHAP panel** renders which features drove the decision.

**Memory section** polls `/api/simulate/verify` every 2.5 seconds (up to 90 s) to confirm all four parallel sinks received the event:

| Sink | What is verified |
|---|---|
| RisingWave | `recent_events` MV includes the event ID |
| Detection + ClickHouse | A detection rule matched and the alert is in ClickHouse |
| Iceberg / MinIO | Event flushed to warm Parquet (`s3://attest-warm`) |
| MCP Gateway | Attestation envelope is queryable by external auditors |

**Batch mode** fires 50 concurrent `POST /triage` calls and returns min/p50/p95/p99/max latency — use this to verify the orchestrator's latency envelope under concurrency.

---

### Load Lab (`/workbench/load`)

A client component backed by the `load-store` Zustand store. Tests the **streaming rules path** (Kafka → RisingWave → Detection Runtime) at configurable throughput.

Opens a WebSocket to `control-plane /v1/metrics/stream` on page load. The WS header shows **"WS live"** (green) when connected. Metrics arrive at 1 Hz as `MetricsSnapshot` frames.

**Four preset profiles:**

| Preset | Rate | Duration | Sampled triage | Purpose |
|---|---|---|---|---|
| Smoke | 1k/sec | 30s | 5% | Sanity check |
| Sustained | 10k/sec | 60s | 5% | Normal load |
| Burst | 100k/sec | 60s | 1% | Stress test |
| 1M Challenge | 100k/sec | 120s | off | 12M events — proves 1M/10s goal |

**Four live charts:**

| Chart | Source field | What it tells you |
|---|---|---|
| Events / sec | `events_per_sec` | Kafka cloudtrail HWM delta |
| Consumer lag | `consumer_lag` | Messages queued but not yet consumed by RisingWave |
| Detections / sec | `detections_per_sec` | Kafka alerts HWM delta — rules firing |
| Storage rows / sec | `clickhouse_rows_per_sec` | ClickHouse warm-tier ingestion rate |

**Session totals panel** shows cumulative events sent, detections fired, actual rate, and consumer lag. If **Sampled triage** (0–20% slider) is enabled, a purple row shows the live ML triage p95 under load.

**Key diagnostic:** if consumer lag grows unboundedly during a Burst run, RisingWave is falling behind. If it stays under ~50k messages, the pipeline is keeping up.

---

### Agents (`/workbench/agents`)

Fetches `orchestrator /metrics` to get `triage_count`, `p50_ms`, `p95_ms`, `p99_ms`. Shows three header counters — Total verdicts, Classifier P99, Envelope coverage.

Four agent cards in a 2×2 grid:

| Agent | Status | Model | When |
|---|---|---|---|
| Triager | Live (if orchestrator up) | XGBoost + local Unsloth / Anthropic escalation | Phases 4a–4b |
| Investigator | Live (if orchestrator + MCP up) | Same inference router as triager (`ATTEST_LLM_PROVIDER`) | Phase 7 — runs on `needs_investigation` |
| Hunter | Shell | — | Phase 8+ (Hunter agent; see `docs/10_Build_Order.md`) |
| Detection Engineer | Phase 10 | Claude Sonnet | Not yet built |

The Triager card shows real-time verdict count and p50/p95/p99 latency percentiles, plus an **"Attestation envelopes: Ed25519 signed"** confirmation badge when the orchestrator is online.

---

### Hunt (`/workbench/hunt`) — query UI shell

A HELIQL-style query area with **Run against warm tier** (proxies to `control-plane /v1/warm/query`), plus stream/save placeholders for the future **Hunter** agent (Phase 8+ per `docs/10_Build_Order.md` / `docs/12_Workbench.md`).

---

### Settings (`/workbench/settings`)

Static page — no backend calls. Shows integration status across four sections: Integrations (CloudTrail, Okta, M365, Arroyo, MinIO, ClickHouse), Detection Rules, AI Agents, and Notifications. Status badges show "connected" (green) or "not configured" (muted). These reflect design intent, not live health checks (use the Admin page for live health).

---

### Admin (`/workbench/admin`)

Four parallel health checks on every page load:

```
control-plane  GET /healthz               → ok | error
arroyo         GET /api/v1/ping           → ok | error
arroyo         GET /api/v1/pipelines      → list of running pipeline names
collector      GET /healthz               → ok | error
```

**Service health table** — 7 rows, each linking to its API docs at `/docs`:

| Service | Port | Type | Links to |
|---|---|---|---|
| control-plane | 8080 | HTTP | `/docs` — Scalar API explorer |
| arroyo | 5115 | HTTP | Arroyo UI |
| collector | 4000 | HTTP | `/docs` — Scalar API explorer |
| orchestrator | 4300 | HTTP | `/docs` — Scalar API explorer |
| mcp-gateway | 4242 | HTTP | `/docs` — Scalar API explorer |
| storage-iceberg | — | Worker | No HTTP — background Kafka consumer |
| detection-runtime | — | Worker | No HTTP — background HELIQL deployer |

**Arroyo pipelines section** — clickable chips with human-readable names (snake_case converted to Title Case) linking to the Arroyo pipeline UI.

---

### OpenAPI Docs (`/docs` on each Rust service)

Every Rust HTTP service serves an interactive [Scalar](https://scalar.com) API explorer at `/docs`. Annotations are generated from `utoipa` macros at compile time — no separate spec file to maintain.

| Service | URL |
|---|---|
| `attest-collector` | `http://localhost:4000/docs` |
| `attest-control-plane` | `http://localhost:8080/docs` |
| `attest-orchestrator` | `http://localhost:4300/docs` |
| `attest-mcp-gateway` | `http://localhost:4242/docs` |

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
| `crates/attest-attestation` | 4a | Ed25519-signed envelopes — `ClassifierEvidence`, `LlmEvidence`, `HybridEvidence`, append-only log |
| `crates/attest-policy-engine` | 4a | Hard-coded per-role tool authorization — `authorize(role, tool_id, ctx) → PolicyDecision` |
| `crates/attest-mcp-gateway` | 4a | Tool call interception service — policy enforcement, argument hashing, call logging |
| `crates/attest-feature-extractor` | 4a | `OcsfEvent` → `AlertFeatures` (8 numeric features) for ML classifiers |
| `crates/attest-onnx-runtime` | 4a | `tract-onnx` inference — `OnnxClassifier` (XGBoost) + `NoveltyDetector` (Mahalanobis) |
| `crates/attest-orchestrator` | 4a | Hybrid triage loop — feature extraction → classify → calibrate → route → attest |

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | 1.78+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Docker + Compose | 24+ | [docker.com](https://docs.docker.com/get-docker/) |
| Python | 3.11+ | [python.org](https://www.python.org/downloads/) |
| uv | latest | `curl -LsSf https://astral.sh/uv/install.sh \| sh` |
| Bun | 1.3+ | `curl -fsSL https://bun.sh/install \| bash` |
| Make | any | pre-installed on macOS/Linux |

---

## Quick Start

### 1. Start infrastructure

```sh
make dev-up-infra
```

Starts Redpanda (`:9092`, `:19092`), RisingWave (`:4566`), Postgres (`:5432`), MinIO (`:9000`, console `:9001`), and ClickHouse (`:8123`) in Docker. The `minio-init` one-shot container creates the `attest-warm` bucket automatically and exits with code 0.

### 2. Start the full platform stack

```sh
make dev-up-services
```

Builds and starts `attest-collector` (`:4000`), `attest-control-plane` (`:8080`), `attest-storage-iceberg`, `arroyo` (`:5115`), and `arroyo-pipeline-deployer` using the multi-stage Dockerfiles in `infra/docker/` and the pre-built Arroyo image.

The `arroyo-pipeline-deployer` one-shot container waits for Arroyo to be healthy, then deploys the SQL pipeline definitions from `infra/arroyo/pipelines/` via the Arroyo REST API.

Open the Arroyo web UI at **http://localhost:5115** to inspect running pipelines and view real-time metrics.

```sh
make arroyo-ui        # opens http://localhost:5115 in your browser
make arroyo-deploy    # (re-)deploy pipelines to a running local Arroyo instance
```

### 3. Verify services are healthy

```sh
curl http://localhost:4000/healthz    # {"status":"ok"}
curl http://localhost:8080/healthz    # {"status":"ok"}
```

### 4. Apply Postgres schema (workbench / Better Auth)

The workbench uses **Postgres** for Better Auth (`auth_*` tables, snake_case columns). Migrations live in **`infra/db`** (Python **uv** + **Alembic**; SQL under `infra/db/sql/`). After infra is up (`make dev-up-infra` or full stack), run:

```sh
cd infra/db
uv sync
export DATABASE_URL=postgres://attest:attest@127.0.0.1:5432/attest
uv run alembic upgrade head
```

This applies the auth DDL and **seeds local dev users** (same password for all: **`analyst-dev`**):

| Email | Notes |
| --- | --- |
| `analyst@attest.local` | Default on the login form |
| `viewer@attest.local` | Extra persona / second browser session |
| `operator@attest.local` | Extra persona |

`apps/workbench/migrations/0001_auth.sql` is a **symlink** into `infra/db/sql/` for tools that still expect that path. **Prefer Alembic** so `alembic_version` stays in sync.

Optional HTTP fallback (not required if migrations ran): `POST /api/auth/seed-analyst` with `ALLOW_AUTH_SEED=1` and header `x-seed-secret` — see `apps/workbench/.env.local.example`.

### 5. Start the workbench UI

```sh
cd apps/workbench
cp .env.local.example .env.local   # then set BETTER_AUTH_SECRET (≥32 chars), DATABASE_URL, etc.
bun install
bun run dev
# Open http://localhost:3000
```

The queue page badge will show **"Live — control-plane connected"** when the backend is reachable. Sign in at `/login` with one of the dev emails above (`analyst-dev`).

### 6. Train the classifier and run the Triager agent (Phase 4a)

```sh
# Install Python ML dependencies and train all artifacts (~30 s)
make train-classifier

# Run E2E tests — auto-starts orchestrator + calibration sidecar
make e2e-phase4a

# Or start services manually for interactive use:
cd ml && uv run python triager/calibrate.py --serve &     # calibration sidecar :5001
ARTIFACTS_DIR=ml/triager/artifacts ORCHESTRATOR_PORT=4300 \
  cargo run -q -p attest-orchestrator &                   # orchestrator :4300

# Triage an alert
curl -s -X POST http://localhost:4300/triage \
  -H 'Content-Type: application/json' \
  -d '{
    "alert_json": {
      "class_uid": 3002,
      "severity_id": 4,
      "time": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
      "actor": {"user": {"name": "alice@example.com"}},
      "metadata": {"product": {"vendor_name": "AWS"}}
    },
    "case_id": "00000000-0000-0000-0000-000000000001",
    "tenant_id": "local-dev"
  }'
# Returns: { "verdict": "...", "confidence": 0.97, "action_id": "...", "latency_ms": 28 }

# Inspect the agent definition and Ed25519 verifying key
curl -s http://localhost:4300/agent | jq .
```

---

## Testing

### Strategy — per-phase E2E, not one giant test

Each phase has its own E2E acceptance gate. This is deliberate:
- **Faster feedback** — a Phase 1 failure doesn't run Phase 4 tests
- **Clearer blame** — a failing test tells you exactly which layer broke
- **Incremental CI** — add phases to CI as they stabilise

Phase 4b (Load Lab, Simulate Lab) is tested manually via the UI. Automated E2E for the load path is covered by Phase 3's detection test. The Simulate Lab's ML path is covered by Phase 4a's orchestrator tests.

---

### Step-by-Step Platform Test (do this after every significant change)

**Prerequisites:** `make dev-up-services` is running, `make train-classifier` has been run once. For workbench routes that require sign-in, apply **Postgres migrations** from `infra/db` (`uv run alembic upgrade head` — see Quick Start §4) and ensure `apps/workbench/.env.local` has `DATABASE_URL` pointing at the same database.

#### 1. Verify the streaming substrate

```sh
# Post a single event
curl -s -X POST http://localhost:4000/ingest \
  -H 'Content-Type: application/json' \
  -d '{"Records":[{"eventName":"ConsoleLogin","eventTime":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "awsRegion":"us-east-1","recipientAccountId":"111111111111",
    "userIdentity":{"type":"IAMUser","userName":"alice@example.com",
    "arn":"arn:aws:iam::111111111111:user/alice","accountId":"111111111111"}}]}'
# Expected: {"event_ids":["<uuid>"]}

# Wait 2s, then verify it's in the hot tier
sleep 2 && curl -s "http://localhost:8080/v1/events/recent?id=<uuid>" | jq .
# Expected: event JSON with actor_user_name: "alice@example.com"

# Verify alice's baseline was updated
curl -s "http://localhost:8080/v1/baselines/user/alice@example.com" | jq .
# Expected: {"regions_seen_30d":["us-east-1"],...}
```

#### 2. Verify the detection rules fire

```sh
# Seed alice with 5 us-east-1 logins to build a baseline
for i in $(seq 5); do
  curl -s -X POST http://localhost:4000/ingest \
    -H 'Content-Type: application/json' \
    -d '{"Records":[{"eventName":"ConsoleLogin","eventTime":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
      "awsRegion":"us-east-1","recipientAccountId":"111111111111",
      "userIdentity":{"type":"IAMUser","userName":"alice@example.com",
      "arn":"arn:aws:iam::111111111111:user/alice","accountId":"111111111111"}}]}'
  sleep 1
done

# Wait 30s for RisingWave to build the materialized view
sleep 30

# Inject a geo-anomaly login from an unexpected region
curl -s -X POST http://localhost:4000/ingest \
  -H 'Content-Type: application/json' \
  -d '{"Records":[{"eventName":"ConsoleLogin","eventTime":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "awsRegion":"ap-southeast-1","recipientAccountId":"111111111111",
    "userIdentity":{"type":"IAMUser","userName":"alice@example.com",
    "arn":"arn:aws:iam::111111111111:user/alice","accountId":"111111111111"}}]}'

# Wait up to 10s for the detection to fire on the alerts topic
docker exec attest-redpanda-1 rpk topic consume alerts --brokers localhost:9092 -n 1
# Expected: JSON with detection_id: "aws_console_login_from_anomalous_geolocation"
```

#### 3. Verify the ML hot-path (Simulate Lab)

```sh
# Start the orchestrator and calibration sidecar (if not running)
cd ml && uv run python triager/calibrate.py --serve &
ARTIFACTS_DIR=ml/triager/artifacts cargo run -p attest-orchestrator &
sleep 5

# Triage a known brute-force alert
curl -s -X POST http://localhost:4300/triage \
  -H 'Content-Type: application/json' \
  -d '{"alert":{"severity_id":4,"class_uid":3002,"entity_reputation_score":0.8}}'
# Expected: {"verdict":"true_positive","calibrated_confidence":>0.5,"latency_ms":<200}

# Or use the Simulate Lab UI:
# 1. Open http://localhost:3000/workbench/simulate
# 2. Select "Brute Force Login" scenario
# 3. Click "Run Simulation" — hot path should complete in < 5s
# 4. Click "Batch (50× concurrent)" — observe p95 latency
```

#### 4. Load Lab — streaming rules at scale

```sh
# Open http://localhost:3000/workbench/load
# Verify: WS live badge is green (control-plane WebSocket connected)

# Click "Smoke" preset (1k/sec, 30s, 5% sampled triage) then Run
# Expected within 5s:
#   - Events/sec chart: ~1000/s
#   - Consumer lag: spikes then drains as RisingWave keeps up
#   - Detections/sec: non-zero after ~10s (geo-anomaly rules fire)
#   - Storage rows/sec: non-zero after first Iceberg flush (~30s)
#   - Triage p95: non-zero after a few seconds (5% sampled to orchestrator)

# For a high-throughput test:
# Click "Burst" preset (100k/sec, 60s, 1% sampled triage) then Run
# Key question: does consumer lag grow unboundedly (RisingWave overloaded)?
# or does it stay < ~50k messages (pipeline keeps up)?

# CLI equivalent — runs without UI:
make load-cli-smoke     # 10k/sec, 30s
make load-cli-burst     # 100k/sec, 60s
make load-cli-attack    # 100k/sec, 60s, attack scenario only
```

#### 5. Verify the full pipeline end-to-end

```sh
# Run all automated E2E tests in sequence
make e2e-phase1    # Streaming substrate
make e2e-phase2    # Iceberg warm tier
make e2e-phase3    # HELIQL detection rules
make e2e-phase4a   # ML triager (self-contained)
make e2e-phase4b   # Hybrid LLM escalation (requires Unsloth on :8888)
make e2e-phase5    # Guardrails (scripted LLM; may auto-start deps per Makefile)
make e2e-phase6    # Shadow check + auto-close
make e2e-phase7    # Investigator loop (WireMock MCP; offline)
# Live Phase 7 (orchestrator + workbench-api + shared ATTEST_LOG_PATH + LLM):
#   ATTEST_E2E=1 ATTEST_PHASE7_LIVE=1 make e2e-phase7-live
```

---

### Unit Tests (no Docker required)

```sh
cargo test --workspace --lib
```

Covers OCSF type round-trips, CloudTrail normalizer, HELIQL parser/compiler, and serialization.

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

### Automated E2E Tests

```sh
# Phase 1 — streaming substrate (requires running stack)
make e2e-phase1
# Posts a ConsoleLogin event → polls recent_events (≤ 5 s) → polls entity_baselines for user region (≤ 30 s).

# Phase 2 — Iceberg warm tier (requires running stack)
make e2e-phase2
# Seeds 10 000 events → waits for Iceberg flush to MinIO (≤ 90 s) → asserts ClickHouse count + GROUP BY aggregate (≤ 30 s).

# Phase 3 — HELIQL detection engine (requires running stack)
make e2e-phase3
# Seeds alice's US-region baseline → injects ap-southeast-1 login → asserts alert on `alerts` Kafka topic within 10 s.
# Also tests: StopLogging event fires a critical alert for aws_cloudtrail_logging_disabled.

# Phase 4a — Hybrid Triager (self-contained — starts and stops its own services)
make e2e-phase4a
# 1. Starts Python calibration sidecar on :5001
# 2. Starts attest-orchestrator on :4300 (with ONNX artifacts from ml/triager/artifacts/)
# 3. triager_classifier_path_produces_attested_verdict — known brute-force alert → classifier path, confidence ≥ 0.5, latency < 500 ms
# 4. triager_classifier_escalates_when_out_of_distribution — OOD alert → hybrid path, escalated_stub verdict
# 5. triager_classifier_p99_latency_under_200ms — 10 requests, P99 < 200 ms
# 6. Stops both services
#
# Prerequisite: make train-classifier (only needed once, or when golden_cases.json changes)

# Phase 4b — Hybrid LLM escalation (requires ATTEST_LLM_PROVIDER=local + Unsloth on :8888)
make e2e-phase4b

# Phase 5 — guardrails (scripted LLM + WireMock)
make e2e-phase5

# Phase 6 — shadow check + auto-close
make e2e-phase6

# Phase 7 — investigator loop (offline; WireMock MCP)
make e2e-phase7

# Phase 7 live — real triage + investigator + trace (needs Unsloth/LLM, MCP, control-plane,
# native orchestrator + workbench-api, identical ATTEST_LOG_PATH in .env)
# ATTEST_E2E=1 ATTEST_PHASE7_LIVE=1 make e2e-phase7-live
```

Phases 1–3 require `make dev-up-services` to be running. Phase 4a is self-contained. Phases 5–7 are Rust E2E crates (`tests/e2e-tests`); see each Makefile target and `scripts/e2e/README.md` for prerequisites. **Phase 7 live** additionally requires two long-running processes (`make run-orchestrator`, `make run-workbench-api`) and a shared `ATTEST_LOG_PATH`.

---

### Hard-won testing lessons (do not repeat)

#### 1. rdkafka `subscribe()` does not assign partitions — the first `recv()` does

**Context:** Any E2E test that creates a `StreamConsumer`, calls `subscribe(&["topic"])`, sleeps for a fixed duration, and then produces events it expects to read back.

**Root cause:** In rdkafka (and the underlying librdkafka), `subscribe()` only registers *intent*. The actual partition assignment — including the commitment of the "latest" offset as the starting position — happens lazily on the first internal poll cycle, which is triggered by the first `recv()` call. A fixed `sleep(2s)` before producing events is not a reliable signal that assignment has completed. If the first `recv()` call happens *after* the message was already written to the broker, that message is silently skipped.

This manifested in `arroyo_cep_pipeline_fires_sequence_alert`: the Arroyo CEP pipeline was working correctly and producing alerts to `alerts` within ~3 seconds, but the test consumer received zero messages for the full 30 s wait window because it was never assigned to any partition before the alert was produced.

**Fix:** After `subscribe()`, call `recv()` with a short timeout to force partition assignment *before* producing the test events. This blocks until the rebalance completes and the consumer is fully seated at the latest offset:

```rust
consumer.subscribe(&["alerts"]).expect("subscribe failed");

// Force eager partition assignment. subscribe() only registers intent;
// the first recv() triggers the rebalance and commits the starting offset.
let _ = tokio::time::timeout(Duration::from_secs(3), consumer.recv()).await;
```

**Broader rule:** Never rely on a fixed sleep after `subscribe()`. Always trigger at least one `recv()` (or `poll()`) before producing events the consumer is meant to read.

#### 2. Arroyo streaming JOINs need continuous watermark advancement, not a one-shot burst

**Context:** `arroyo_cep_pipeline_fires_sequence_alert` — CEP pipeline detecting `ConsoleLogin → GetObject` sequences.

**Root cause:** Arroyo's interval JOIN emits results once the watermark advances past the join window boundary. Sending 5 watermark-advance events in a 500 ms burst and then waiting 30 s is fragile — if Arroyo's internal processing lags or the watermark does not advance enough from those 5 events, the join never emits inside the test window.

**Fix:** Pump one watermark-advance event every 500 ms *throughout the entire polling loop*, not just before it. This guarantees the watermark keeps advancing regardless of pipeline lag:

```rust
let deadline = Instant::now() + Duration::from_secs(60);
let mut pump_tick = Instant::now();
let mut pump_idx: u32 = 0;

while Instant::now() < deadline && !found {
    if pump_tick.elapsed() >= Duration::from_millis(500) {
        post_cloudtrail(&client, "ConsoleLogin",
            &format!("watermark-advance-{}@example.com", pump_idx), "eu-west-1").await;
        pump_idx += 1;
        pump_tick = Instant::now();
    }
    // ... poll consumer.recv() with 200ms timeout ...
}
```

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
│   ├── attest-detection-runtime/ # Detection deploy + poll + emit to alerts topic (Phase 3)
│   ├── attest-attestation/       # Ed25519 envelopes + append-only log (Phase 4a)
│   ├── attest-policy-engine/     # Per-role tool authorization policies (Phase 4a)
│   ├── attest-mcp-gateway/       # Tool call intercept service :4242 (Phase 4a)
│   ├── attest-feature-extractor/ # OcsfEvent → AlertFeatures vector (Phase 4a)
│   ├── attest-onnx-runtime/      # tract-onnx classifier + Mahalanobis novelty (Phase 4a)
│   └── attest-orchestrator/      # Hybrid triage loop :4300 (Phase 4a)
├── ml/
│   └── triager/                  # Python ML pipeline (uv-managed)
│       ├── golden_cases.json     # 210 labelled alerts (200 in-dist + 10 OOD)
│       ├── train.py              # XGBoost → model.onnx + shap_background.npy
│       ├── novelty.py            # Mahalanobis → novelty_mean/inv_cov.npy + threshold
│       ├── calibrate.py          # Isotonic regression → calibration_models.pkl + FastAPI sidecar
│       ├── Makefile              # train / novelty / calibrate / serve-calibration targets
│       └── artifacts/            # Generated — gitignored
├── agents/
│   ├── triager-hybrid-v1.json    # Versioned agent definition with artifact SHA-256 hashes
│   └── triager.system_prompt.md  # LLM system prompt (Phase 4b)
├── eval/
│   ├── golden_cases/             # Evaluation datasets
│   └── run_eval.py               # Calls POST /triage for each golden case, asserts metrics
├── detections/                   # 10 bundled HELIQL detection rules (Phase 3)
├── apps/
│   └── workbench/                # Next.js 16 marketing site + SOC workbench UI
├── infra/
│   ├── db/                       # Postgres migrations (uv + Alembic) — Better Auth schema + dev seed users
│   ├── arroyo/                   # Arroyo streaming engine
│   │   ├── pipelines/            # SQL pipeline definitions deployed via REST API
│   │   │   ├── cloudtrail_to_parquet.sql    # Redpanda → Parquet → MinIO ETL
│   │   │   └── cep_sequence_detection.sql   # Login → S3-access sequence (CEP)
│   │   └── deploy-pipelines.sh   # curl-based idempotent pipeline deployer
│   ├── clickhouse/               # ClickHouse config (listen + S3/MinIO access)
│   ├── docker/                   # Multi-stage Dockerfiles
│   ├── risingwave/               # RisingWave DDL (phase1_baseline.sql)
│   └── terraform/                # IaC (Phase 5+)
├── tests/
│   └── e2e-tests/                # E2E integration tests (ATTEST_E2E=1 required)
│       └── tests/
│           ├── phase1_streaming.rs
│           ├── phase2_iceberg.rs
│           ├── phase3_detection.rs
│           ├── phase4a_triager.rs
│           ├── phase4b_llm_escalation.rs
│           ├── phase5_guardrails.rs
│           ├── phase6_auto_close.rs
│           ├── phase7_live_investigator.rs
│           └── phase_arroyo_pipelines.rs
├── docs/                         # Source-of-truth documentation
├── docker-compose.yml            # Local dev stack
└── Makefile                      # Convenience targets
```

---

## Make Targets

| Target | What it does |
|---|---|
| `make dev-up-infra` | Start core infra (Redpanda, RisingWave, Postgres, MinIO + init, ClickHouse) |
| `make dev-up-services` | Start app services only — assumes infra is already running |
| `make dev-up-all` | ⭐ Start everything in order: infra → init → all services (clean fresh start) |
| `make dev-down-infra` | Stop core infrastructure containers only |
| `make dev-down-all` | Stop and remove ALL containers (infra + services) |
| `make arroyo-ui` | Open Arroyo web UI at http://localhost:5115 |
| `make arroyo-deploy` | (Re-)deploy SQL pipelines to a running local Arroyo instance |
| `make e2e-arroyo` | Run Arroyo E2E tests — health, pipeline deploy, ETL Parquet, CEP alert (requires `dev-up-services`) |
| `make train-classifier` | Run full ML pipeline — `train.py` + `novelty.py` + `calibrate.py` via `uv` |
| `make e2e-phase1` | Run Phase 1 E2E test (requires `ATTEST_E2E=1` + running stack) |
| `make e2e-phase2` | Run Phase 2 E2E test (requires `ATTEST_E2E=1` + running stack) |
| `make e2e-phase3` | Run Phase 3 E2E test (requires `ATTEST_E2E=1` + running stack) |
| `make e2e-phase4a` | Start calibration sidecar + orchestrator, run Phase 4a E2E tests, stop services |
| `make e2e-phase4b` | Phase 4b hybrid LLM escalation E2E (requires local LLM / Unsloth on :8888) |
| `make e2e-phase5` | Phase 5 guardrails E2E (`tests/e2e-tests/tests/phase5_guardrails.rs`) |
| `make e2e-phase6` | Phase 6 shadow check + auto-close E2E |
| `make e2e-phase7` | Phase 7 investigator integration test (offline; WireMock MCP) |
| `make e2e-phase7-live` | Phase 7 live: real `POST /triage` + investigator + trace (needs stack + `ATTEST_E2E=1` + `ATTEST_PHASE7_LIVE=1`) |
| `make run-orchestrator` | Run hybrid triage + investigator orchestrator natively (`:4300`) |
| `make run-workbench-api` | Run attestation trace API (`:4400`; reads `ATTEST_LOG_PATH`) |
| `make load-gen-up` | Start load-gen container (bench profile) |
| `make load-cli-smoke` | CLI benchmark: 10k/sec · 30s · mixed · seed baselines |
| `make load-cli-burst` | CLI benchmark: 100k/sec · 60s · mixed |
| `make load-cli-attack` | CLI benchmark: 100k/sec · 60s · attack scenario |
| `make load-status` | GET load-gen /status (running metrics) |
| `make load-stop` | POST load-gen /stop |
| `make fmt` | `cargo fmt --all` |
| `make lint` | `cargo clippy` + `bun run lint` |
| `make smoke` | Quick smoke check — cargo test + bun test + pytest |
| `make dev-up-llm` | Start core infra + llama.cpp (requires Qwen GGUF volume-mounted) |

---

## Railway Deployment

### Service Map

| Railway service | Image / Builder | Internal hostname |
|---|---|---|
| `redpanda` | `confluentinc/cp-kafka:7.7.8` (KRaft mode — named `redpanda` so no app env vars change) | `redpanda.railway.internal:9092` |
| `risingwave` | `risingwavelabs/risingwave:latest` | `risingwave.railway.internal:4566` |
| `clickhouse` | `clickhouse/clickhouse-server:latest` | `clickhouse.railway.internal:8123` |
| `minio` | `quay.io/minio/minio:latest` | `minio.railway.internal:9000` |
| `arroyo` | `ghcr.io/arroyosystems/arroyo:latest` | `arroyo.railway.internal:5115` |
| `arroyo-deployer` | Dockerfile `infra/docker/arroyo-deployer.Dockerfile` | one-shot (exits after deploying pipelines) |
| `collector` | Dockerfile `infra/docker/collector.Dockerfile` | — |
| `control-plane` | Dockerfile `infra/docker/control-plane.Dockerfile` | `control-plane-production-b6e3.up.railway.app` |
| `storage-iceberg` | Dockerfile `infra/docker/storage-iceberg.Dockerfile` | — |
| `detection-runtime` | Dockerfile `infra/docker/detection-runtime.Dockerfile` | — |
| `workbench` | Nixpacks (`nixpacks.toml`) | `workbench-production-6e86.up.railway.app` |

### First-time setup

```sh
railway login
make railway-infra        # create the 5 Docker-image infrastructure services (incl. Arroyo)
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
Redpanda's Seastar I/O engine requires `perf_event_open` syscall and Linux AIO — both blocked in Railway's container sandbox. Redpanda starts, passes the health check, then crashes within ~10 seconds. **Use `confluentinc/cp-kafka:7.7.8` (KRaft mode) instead.** It uses standard Java I/O and runs fine. The service is still named `redpanda` so no app env vars need updating.

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
`bitnami/kafka:latest` does not exist — use `bitnami/kafka:3.9` or a specific version. Alternatively, use `confluentinc/cp-kafka:7.7.8` (which is what this project uses) or `apache/kafka:latest` (official image, does have `latest`).

#### 10. Detection rules baked into the Docker image
Railway does not support local volume mounts from the host. Detection rules (`.heliql` files) are copied into the `detection-runtime` image at build time via `COPY detections/ /rules/` in `infra/docker/detection-runtime.Dockerfile`. The `RULES_DIR=/rules` env var is set in the Dockerfile. This is intentional and correct for Railway deployments.

#### 11. RisingWave start command must include the binary
Railway's start command replaces the image entrypoint, not just `CMD`. Use `/risingwave/bin/risingwave single_node` (underscore). Bare `playground` or `single-node` fails with no container logs. Current images default to `single_node`.

#### 12. `railway up` and gitignored ML artifacts
`railway up` respects `.gitignore`, so `ml/triager/artifacts/` is omitted from the upload. Orchestrator and calibration-sidecar Dockerfiles `COPY` those files — without them the Metal builder dies at schedule with almost no logs. Deploy those two services with `railway up --no-gitignore`. `.railwayignore` still excludes `target/`, `node_modules/`, and `.venv/` so the archive stays small.
