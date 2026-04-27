# 07 — Revised Tech Stack

**Product:** Attest
**Document type:** Authoritative tech stack decisions. Supersedes the stack notes in `02_Architecture.md` where they differ.

---

## 1. Stack at a glance

| Layer | Original (02_Architecture) | **Revised (this document)** | Rationale |
|---|---|---|---|
| Hot path / collectors | Rust | **Rust** | Unchanged |
| Streaming backbone | Kafka or Redpanda | **Redpanda** (single binary, no JVM, no ZooKeeper) | Aligned with all-Rust ops; familiar to author |
| Stream runtime — primary | Flink + RisingWave | **RisingWave** (Rust, streaming SQL, Postgres protocol) | Drops JVM; SQL is what detection engineers know |
| Stream runtime — stateful CEP | Flink (DataStream API) | **Arroyo** (Rust, SQL streaming, Flink replacement) | All-Rust; Cribl acquired Arroyo, which validates the security use case |
| Stream runtime — extreme cases | Flink | **Custom Rust services on Redpanda** | The 5% edge case Arroyo can't express; written in Rust with Tokio |
| Hot OLAP | ClickHouse or Druid | **ClickHouse only** | Drop Druid; one fewer system to operate |
| Warm query engine | Trino over Iceberg | **ClickHouse via Iceberg table function** | Drop Trino (JVM); ClickHouse can read Iceberg directly |
| Warm storage | Iceberg on S3/GCS/ABFS | **Iceberg on S3 (prod) / MinIO (local)** | Same Iceberg code path locally and in prod |
| Cold storage | S3 Glacier | **S3 Glacier** (prod) / MinIO with lifecycle policy (local) | Unchanged in prod |
| Control plane API | Go | **Rust** (axum + tokio) | All-Rust except where Python is needed for ML |
| ML / inference services | Python | **Python** (only where needed: model inference, training, calibration) | Unchanged — Python is the right tool for ML |
| Agentic plane runtime | Python (Anthropic SDK) | **Rust orchestrator + Python ML sidecar** | Rust handles MCP gateway, policy, attestation; Python handles ML |
| LLM inference — local dev | (not specified) | **llama.cpp + Qwen 3 ~7B** | Local Triager; matches author's tooling |
| LLM inference — prod | Anthropic, OpenAI, Bedrock, self-hosted | **Anthropic primary; multi-provider router; Qwen 3 32B via vLLM for air-gapped** | Routed per agent role |
| Frontend | React/TypeScript | **Next.js (App Router) with WebSockets** | Author's stack; SSR + RSC for the workbench shell, WS for live streams |
| Workbench API | Go | **Rust (axum) for REST, separate WS gateway in Rust** | Same all-Rust story |
| Local dev object store | (not specified) | **MinIO** (S3-compatible) | Local-first parity with prod S3 |
| Deployment — early/MVP | (not specified) | **Railway** for Attest-managed components | Fast iteration, low ops |
| Deployment — regulated/scale | BYOC on AWS/GCP/Azure | **BYOC on AWS/GCP/Azure** | Unchanged |
| GitOps + IaC | Pulumi + ArgoCD + Kubernetes | **Pulumi (BYOC) + Railway template (MVP)** | Two paths: Railway for managed, full K8s for BYOC |

## 2. Why dropping Flink does not compromise the solution

This is the most consequential change. Here is the honest assessment, written so a skeptic on the team can challenge it.

### 2.1 What Flink uniquely gives that Rust alternatives do not match

- **FlinkCEP** for n-event sequence patterns with arbitrary conditional logic and quantifiers. Example: *"login from new country THEN access to S3 bucket WITHIN 5 min THEN bulk download WITHIN 10 min"* with full pattern composition.
- **Battle-tested exactly-once at petabyte scale** (Uber, Netflix, Pinterest references).
- **Largest ecosystem of mature connectors** for legacy enterprise sources.

### 2.2 What we substitute, and what coverage we get

| Detection pattern | Coverage in revised stack | Tool |
|---|---|---|
| Threshold / windowed aggregation | 100% | RisingWave (SQL `OVER` + tumbling windows) |
| Cross-stream join (auth × network) | 100% | RisingWave temporal joins |
| Per-entity stateful baseline (UEBA) | 100% | RisingWave materialized views + ML sidecar |
| Sequence pattern, 2-event (A then B) | 100% | RisingWave or Arroyo |
| Sequence pattern, 3–4 event with simple conditions | ~95% | Arroyo |
| Sequence pattern, n-event with rich conditional CEP | ~70% in Arroyo | Custom Rust service for the 5% gap |
| ML inference in stream | 100% | Python sidecar via gRPC; Rust ONNX runtime for hot path |
| Replay from durable buffer | 100% | Redpanda tiered storage |
| Exactly-once semantics | 100% | RisingWave (transactional) + Arroyo (checkpointing) |

The honest gap: maybe **5–10% of detection expressiveness** for very complex multi-stream stateful CEP. We close that gap with custom Rust services consuming Redpanda directly using `rdkafka` or `fluvio` clients — typically 200–500 lines of Tokio code per detection class.

### 2.3 What we gain by dropping Flink

- **No JVM in the runtime.** No GC pauses, no heap tuning, no `flink-conf.yaml` rituals.
- **3–5× lower memory footprint** at equivalent throughput. Direct Railway cost win.
- **Faster local dev.** Single-binary services start in seconds vs. 30–60s for a Flink JobManager.
- **SQL-first.** Detection engineers know Postgres SQL; nobody outside Flink shops knows DataStream API.
- **Smaller team needed.** One Rust engineer can operate the whole stream runtime; Flink at scale typically needs a dedicated platform engineer.

### 2.4 The decision

**Skip Flink in MVP and GA v1.** Revisit only if a design partner has a documented detection requirement Arroyo + custom Rust cannot meet. None of the design-partner ICPs surface this requirement based on current research.

## 3. Why dropping Trino is the right call

Trino is the dominant Iceberg query engine, but it is heavy and JVM-based. Two facts make it unnecessary for Attest:

1. **ClickHouse can read Iceberg directly** via its `iceberg` table function (also available as `iceberg('s3://...')` and via the ClickHouse Iceberg integration). Performance for analytical queries on OCSF-normalized warm-tier data is sufficient for the SOC use case; we are not running interactive 10-billion-row joins.
2. **One query engine across hot and warm** dramatically simplifies operations. Detection engineers write one dialect of SQL. Cache layers and query planning are unified.

**Tradeoff:** ClickHouse's Iceberg support is newer than Trino's. We accept this risk in exchange for operational simplicity. If a customer pushes a workload that genuinely requires Trino-class federation, we add Trino as an opt-in component in BYOC deployments. We do not run it by default.

## 4. Why dropping Druid is straightforward

Druid was an alternative hot OLAP for time-series-heavy customers. ClickHouse handles the same workload well enough for any Attest customer; Druid is simply one more system to operate. Cut.

## 5. Local dev stack — full reproducibility

The single biggest engineering velocity decision: every developer can run the entire Attest stack on a laptop in under 5 minutes via `docker compose up`. The stack:

```
┌──────────────────────────────────────────────────────────────────┐
│                       Local dev environment                      │
├──────────────────────────────────────────────────────────────────┤
│  Next.js dev server (frontend)                       :3000       │
│  Attest REST API (Rust + axum)                        :8080       │
│  Attest WebSocket gateway (Rust)                      :8081       │
│                                                                  │
│  Redpanda (single binary, Kafka API)                 :9092       │
│  RisingWave (Postgres protocol)                      :4566       │
│  Arroyo (Web UI + API)                               :5115       │
│  ClickHouse (HTTP + native)                          :8123, 9000 │
│  MinIO (S3-compatible, console)                      :9000, 9001 │
│  Postgres (control plane state)                      :5432       │
│                                                                  │
│  llama.cpp HTTP server with Qwen 3 ~7B               :8088       │
│  Python ML sidecar (FastAPI + ONNX runtime)          :8000       │
└──────────────────────────────────────────────────────────────────┘
```

Iceberg lives on MinIO with the same paths the prod S3 buckets use. ClickHouse reads Iceberg from MinIO via the same `iceberg(...)` function it uses against S3 in prod. **Same code path.**

A single `docker-compose.yml` plus `.env` brings this up. CI runs the same compose stack for integration tests. No mocks, no in-memory fakes — the runtime is the runtime.

## 6. Railway deployment topology

Railway is excellent for MVP through ~10 design partners. Recommended topology:

```
Railway project: Attest-control-plane
  ├─ control-plane-api          (Rust)
  ├─ ws-gateway                 (Rust)
  ├─ frontend                   (Next.js)
  ├─ postgres                   (Railway managed)
  └─ redis                      (sessions, rate limiting)

Railway project: Attest-data-plane-tenant-{id}
  ├─ redpanda                   (single broker for MVP; add brokers as needed)
  ├─ Attest-collector-ingester   (Rust)
  ├─ risingwave                 (single binary, persistent volume)
  ├─ arroyo                     (Rust)
  ├─ clickhouse                 (persistent volume)
  └─ ml-sidecar                 (Python + FastAPI)

External (managed):
  ├─ S3 (warm Iceberg, cold Glacier)        — AWS account, even for Railway customers
  └─ Anthropic API                          — primary LLM provider

Local-only (developer laptops, CI):
  ├─ MinIO replaces S3
  └─ llama.cpp + Qwen replaces Anthropic API
```

**Constraints to plan around:**

- Railway's persistent volumes are sufficient for hot ClickHouse but not designed for petabyte-scale storage. Warm Iceberg lives on S3 even when compute lives on Railway.
- Railway compliance posture (SOC 2 yes; HIPAA/FedRAMP — verify with current state at sign-up) determines whether we can put a regulated customer on Railway. **For HIPAA, financial services, or defense customers, BYOC on AWS/GCP from day one.**
- Multi-region today is limited; cross-region failover is a manual exercise. Acceptable for MVP; a Q3 problem.

**Migration path to BYOC:** identical container artifacts, identical Iceberg layout. The Pulumi modules from `02_Architecture.md` deploy the same images to a customer's EKS/GKE/AKS. Railway is not a lock-in; it is acceleration.

## 7. LLM inference strategy

### 7.1 Per-agent model selection (revised)

| Agent | MVP (months 0–6) | GA v1 (months 6–12) | Air-gapped (year 2) |
|---|---|---|---|
| **Triager** | Local: Qwen 3 ~7B via llama.cpp · Prod: Claude Sonnet via API | Same | Qwen 3 14B via vLLM |
| **Investigator** | Claude Sonnet/Opus via API | Same | Qwen 3 32B via vLLM |
| **Hunter** | Claude Opus via API | Same | Qwen 3 32B via vLLM |
| **Detection Engineer** | Claude Sonnet via API | Same | Qwen 3 32B via vLLM |
| **Responder** | Claude Sonnet (constrained, low temperature) | Same | Qwen 3 14B |
| **Coordinator (planner)** | Claude Sonnet | Same | Qwen 3 14B |

The local llama.cpp+Qwen path is **production-ready for the Triager** because Triager workload is high-volume, narrow-scope, and benefits from small + fast over best-in-class reasoning. For deeper agents, even the largest open-weights models lag Claude Opus on multi-step reasoning meaningfully enough that we recommend API by default and only fall back to open-weights for sovereignty constraints.

### 7.2 Inference router (Rust component)

A thin Rust service routes per-agent inference requests to the appropriate provider. Speaks the OpenAI chat-completions wire format on the inbound side (so any agent code can target one URL) and translates outbound to Anthropic, OpenAI, Bedrock, vLLM, or llama.cpp. Handles retries, fallback (e.g., if Anthropic is rate-limited, fall back to Bedrock), and per-tenant cost accounting.

### 7.3 Cost containment

LLM cost per case is the second-biggest existential risk after hallucination (per `02_Architecture` risks table). Mitigations:

- Tiered agents (Triager handles 80%, cheap by design).
- **Semantic caching** of repeated triage patterns — if a Triager has seen near-identical alert+context pair in the last 24h, return the cached verdict (with new attestation referencing the cached basis).
- **Prompt compression** for long context — RAG retrieval over historical cases for the 1–3 most relevant precedents instead of dumping the full case history.
- **Local Qwen for Triager** in air-gapped or extreme-cost-sensitive customer deployments cuts per-action cost by 95%+.

## 8. Frontend revisions: Next.js + WebSockets

The original architecture said "React/TypeScript SPA, Go API." The revised stack is **Next.js (App Router) + Rust API + Rust WebSocket gateway**.

### 8.1 What goes over WebSocket vs REST vs SSE

The decision rule: **WebSocket for bidirectional + frequent + ephemeral; REST for stable + cacheable; SSE where one-way streaming is enough.**

| Surface | Transport | Why |
|---|---|---|
| Live alert stream into the queue | **WebSocket** | Bidirectional (filter changes, ack) + frequent + ephemeral |
| Active case workbench | **WebSocket** | Multi-user collaboration, live agent updates |
| Live agent reasoning stream (Investigator working in real time) | **WebSocket** (SSE acceptable) | Per-token if needed; subscription-style |
| Detection coverage map | REST + cache | Stable, expensive to compute, polled hourly |
| Reasoning trace replay (already-stored) | REST | Stable, cacheable |
| NL query → DSL | REST | Single request/response |
| Detection authoring (Git PR creation) | REST | Single request/response |
| Long-running operations: backtest, hunt | **WebSocket** for progress + REST for kickoff | Progress is naturally streaming |

### 8.2 WebSocket gateway architecture

A dedicated Rust process (axum + `tokio-tungstenite`) sits between the frontend and Redpanda. It:

- Authenticates connections via JWT on the upgrade handshake.
- Subscribes to per-tenant Redpanda topics filtered by user RBAC.
- Fans out events to connected clients.
- Buffers per-client to handle slow clients without backpressuring the stream.
- Emits heartbeats; closes idle connections after configurable timeout.

Scale model: each gateway instance handles ~10K concurrent connections on a 4-core box. Horizontal scale by sticky-load-balancing on tenant ID.

### 8.3 Next.js App Router specifics

- **Server Components** for the case workbench shell — the structure (case header, evidence panel layout, agent attestation viewer scaffold) is rendered on the server.
- **Client Components** for everything that subscribes to the WebSocket stream — alert queue, live reasoning panel, in-progress backtest indicator.
- **Route handlers** as the lightweight proxy to the Rust REST API for browser-side calls that need session cookies.
- **Vercel-hosted** in MVP (free tier acceptable for design partners) → Railway-hosted for prod control plane → BYOC option in customer cloud for regulated tenants.

### 8.4 Performance budget for the workbench

- Time to interactive (TTI) for case workbench: ≤ 1.5s on a typical SOC analyst's machine.
- Live agent token stream latency: ≤ 200ms from token emission to render.
- Reasoning trace replay scrub: ≤ 50ms per step.

These are aggressive but achievable with Server Components + Edge runtime + a Rust WS gateway.

## 9. Updated build vs buy

Same intent as the original; updated for the revised stack.

| Component | Decision | Rationale |
|---|---|---|
| Streaming backbone | **Buy** (Redpanda) | Single binary, Kafka-compatible, no JVM |
| Stream runtime | **Buy** (RisingWave + Arroyo) | Both Rust, both proven |
| Hot OLAP | **Buy** (ClickHouse) | Reads Iceberg directly; warm + hot in one system |
| Warm storage | **Use open standard** (Iceberg) | Customer-portable; no lock-in |
| Object store local | **Buy** (MinIO) | S3-compatible; same code path as prod |
| Object store prod | **Use cloud** (S3 / GCS / ABFS) | Customer cloud in BYOC |
| Edge collector | **Build** (Rust) | Differentiating; footprint matters |
| Detection DSL & compiler | **Build** | Core IP — the portable detection promise |
| Agentic plane orchestrator | **Build** (Rust) | Core IP — Verifiable Agentic SOC |
| ML sidecar | **Build** (Python) | Thin wrapper around pretrained models |
| Workbench frontend | **Build** (Next.js) | Core IP — UX moat |
| WebSocket gateway | **Build** (Rust) | Performance-critical |
| LLM inference | **Buy multi-provider** | Anthropic primary; vLLM + llama.cpp for sovereignty |
| MCP gateway | **Build** (Rust thin layer) | Need policy enforcement integrated |
| SOAR connector | **Integrate** (Torq, Tines, XSOAR) | Don't fight SOAR vendors |
| GitOps | **Buy** (Pulumi for BYOC; Railway native for MVP) | Standard tooling |

## 10. Updated performance budget

Targets unchanged from `02_Architecture.md` Section 12, but achievable with smaller infrastructure footprint due to the Rust-native stack:

| Path | Target P99 | Estimated infrastructure cost on Railway (per tenant, modest scale) |
|---|---|---|
| Edge collector → ingest backbone | 200 ms | Customer-side, n/a |
| Ingest → in-stream detection alert | 5 s | ~$80/mo (Redpanda + RisingWave) |
| Hot-tier query (24h, single entity) | 1 s | ~$120/mo (ClickHouse) |
| Warm-tier query (30d, aggregate) | 30 s | S3 + ClickHouse compute on-demand |
| Triager agent verdict | 30 s | Anthropic API: ~$0.005/case · Qwen local: <$0.001 |
| Investigator agent verdict | 3 min | Anthropic API: ~$0.15/case |
| Detection backtest (90d, single rule) | 5 min | ClickHouse compute on demand |

A typical mid-market design partner runs Attest on Railway for **$800–$1,500/month of platform cost** plus LLM API spend (~$2K–$5K/month at MVP volumes). That's ~$50K/year platform cost to support a $400–$800K ACV deal — gross margins remain healthy even before scale.

## 11. Migration path from Railway to BYOC

The same image artifacts deploy to either Railway or a customer's EKS/GKE/AKS. The migration story:

1. Customer signs paid contract; if regulated, BYOC clause in the order form.
2. Pulumi modules deploy data plane to customer's VPC.
3. Control plane stays on Railway (Attest-managed) or moves to Attest's AWS account (Year 2 SaaS hardening).
4. Customer's S3 bucket already exists — Iceberg metadata refers to their bucket from day one when they're on Railway too.
5. Cutover is a Redpanda mirror + ClickHouse re-ingest job; ~24 hours of dual-write.

**No re-platforming. No data migration of warm tier (it's already in customer's S3).** This is a key technical-credibility story for design partners.

## 12. Summary of risk changes vs. original architecture

| Risk | Direction | Notes |
|---|---|---|
| Detection expressiveness gap (no Flink) | ⬆ slight increase | Mitigated by Arroyo + custom Rust |
| Operational complexity | ⬇ significant decrease | All-Rust runtime; no JVM |
| Local dev velocity | ⬇⬇ major decrease in friction | Full stack runs on a laptop |
| LLM cost per case | ⬇ moderate decrease | Local Qwen for Triager; semantic cache |
| Railway scale ceiling | ⬆ new risk | Mitigated by clean BYOC migration path |
| Iceberg-via-ClickHouse maturity | ⬆ new risk | Mitigated by Trino opt-in if a customer demands it |
| Frontend WebSocket scale | new consideration | Mitigated by Rust WS gateway design |

Net: **the revised stack is materially better for Attest's stage, team size, and economics, at the cost of a tractable detection-expressiveness gap that affects ~5% of detection patterns.**
