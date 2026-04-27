# 02 — Technical Architecture

**Product:** Attest
**Document type:** Architecture overview, intended for engineering leadership and design-partner CTOs.

> **A note on this document.** This is the canonical architecture overview — the six-plane model, the data flow, the agentic plane structure, the deployment topology. For specific stack decisions (which streaming engine, which warm-tier query engine, which LLM provider), see `07_Stack_Revised.md` — its decisions supersede earlier text where they differ. For the agent harness implementation specification (execution paths, attestation envelope variants, calibration), see `09_Agent_Harness.md`. For ML model strategy and the hybrid Triager design, see `08_Datasets_and_ML.md`. For the analyst-facing workbench (Next.js stack, UI/UX principles, information architecture, critical flows, observability), see `12_Workbench.md`. This document was kept in sync with those four as of the current revision.

---

## 1. Architectural principles

These principles drive every component decision. When in doubt, fall back to them.

1. **Streaming-first, not stream-flavored.** Data is processed in motion by default. Storage is a tier of the stream, not the other way around. This is the difference between Abstract Security's approach and a legacy Splunk pretending to be real-time.

2. **Composable by contract.** Every component speaks open standards (OCSF, OTEL, MCP, Sigma, Iceberg). Customers can replace any block. Lock-in is not a moat; it is technical debt charged to the buyer.

3. **Detection is portable.** The same detection logic must execute in stream, in history, and federated to external engines. Detection logic decoupled from data location is the architectural breakthrough.

4. **AI is a layer, not a feature.** Agentic capabilities run on a separate, governed plane that observes and acts on the data plane. Agents do not have unaudited access to customer data; the policy engine does.

5. **Verifiability is a load-bearing requirement.** Every agent decision is reproducible from a signed trace. This is non-negotiable for the Verifiable Agentic SOC differentiator.

6. **Polyglot performance, with a strong default.** Rust on every hot path and on the control plane. Python where ML lives. TypeScript / Next.js on the client. The all-Rust default for services keeps operational complexity low and matches the team profile; we deviate only where a different language is the demonstrably better tool. (See `07_Stack_Revised.md` for the canonical stack decisions.)

7. **Tenant isolation is physical where possible.** Per-tenant encryption keys, per-tenant compute pools for sensitive workloads. Multi-tenant SaaS only where the math works.

8. **Open table formats. Customer-owned data.** Iceberg + OCSF means a customer can leave Attest in 24 hours and still query their data with any compatible engine — DuckDB, Snowflake, BigQuery, Spark, even Trino if they prefer it. Trust is built on this commitment.

## 2. Logical architecture

The platform is organized into **six planes**, each independently scalable and replaceable.

```
+------------------------------------------------------------------+
|                        WORKBENCH PLANE                           |
|   Next.js · Rust REST API · Rust WS gateway · Trace replay       |
+------------------------------------------------------------------+
                              ▲
+------------------------------------------------------------------+
|                       AGENTIC PLANE (VAS)                        |
|  Coordinator · Triager (Hybrid) · Investigator · Hunter ·        |
|  Detection-Eng · Responder                                       |
|  Policy engine · MCP gateway · Attestation · Calibration         |
+------------------------------------------------------------------+
                              ▲
+------------------------------------------------------------------+
|                       DETECTION PLANE                            |
|  HELIQL → Stream (RisingWave / Arroyo) | Batch (ClickHouse over  |
|  Iceberg) | Federated (push-down)                                |
|  Sigma compiler · UEBA · ML plug-ins · Coverage map              |
+------------------------------------------------------------------+
                              ▲
+------------------------------------------------------------------+
|                       STORAGE PLANE                              |
|  Hot (ClickHouse) · Warm (Iceberg, BYOC) · Cold (S3 Glacier)     |
+------------------------------------------------------------------+
                              ▲
+------------------------------------------------------------------+
|                       STREAMING PLANE                            |
|  Redpanda · Schema registry · Rust edge collector · OCSF         |
|  Shaping, enrichment, routing, replay buffer                     |
+------------------------------------------------------------------+
                              ▲
+------------------------------------------------------------------+
|                        CONTROL PLANE                             |
|  Identity · Multi-tenancy · Billing · Audit · Policy · GitOps    |
|  Railway (MVP) · Terraform + EKS/GKE/AKS (BYOC)                     |
+------------------------------------------------------------------+
```

The **agentic plane never reaches into the storage plane directly.** It calls into the detection plane and a governed query API. This is what makes the agents auditable: every byte they touch is mediated by a policy-enforcing gateway.

## 3. Streaming plane

### 3.1 Edge collection

**Component:** `Attest-collector` (Rust, single static binary, sub-50MB image).

**Responsibilities:**

- Native support for top 50 sources (cloud control planes, EDR, IdP, SaaS, syslog, OTEL, OCSF native, JSON, CEF).
- Edge shaping before any billing meter: filter, aggregate, redact (PII, secrets), enrich (geoip, IAM mapping, asset criticality), transform (regex, JSONPath, custom WASM).
- mTLS to ingest endpoints; persistent on-disk buffer for network outages.
- Backpressure-aware; never drops data without acknowledgment.

**Why Rust:** Mohamed's prior work showed 3.75× throughput, 13× lower memory vs. JVM equivalents. Edge collection is the unforgiving hot path; latency and footprint are non-negotiable.

### 3.2 Ingest backbone

**Component:** Redpanda (single binary, no JVM, no ZooKeeper, Kafka-protocol-compatible) with Schema Registry (OCSF schemas + customer extensions). Customers who already operate Apache Kafka can point Attest collectors at their cluster — the wire protocol is identical.

**Topology:**

- One ingest topic per source class (e.g., `cloudtrail`, `okta`, `crowdstrike`).
- Compaction on reference data; time-partitioned on event data.
- Tiered storage in Redpanda for short-term durability (24–72h) before warm-tier handoff.
- Per-tenant ACLs and per-topic encryption.

**Replay:** every event is durably available for at least 7 days, addressable by `(tenant, topic, offset)` or `(tenant, time-range, source)`.

### 3.3 Pipeline runtime

**Components:** Two Rust-native streaming runtimes, plus a custom-Rust escape hatch.

- **RisingWave** (Rust, Postgres protocol, streaming SQL with materialized views) is the primary engine. It handles ~85% of detection patterns: temporal joins, windowed aggregations, per-entity baselines, threshold rules, sequence patterns up to 2–3 events. Detection engineers already know SQL; nobody knows Flink DataStream API.
- **Arroyo** (Rust, SQL streaming, Cribl-acquired Flink replacement) handles stateful complex event processing — n-event sequence patterns with rich conditional logic that exceed RisingWave's expressiveness.
- **Custom Rust services on Redpanda** (Tokio + `rdkafka` or `fluvio`) cover the genuine ~5% of detection patterns neither engine expresses well. Typically 200–500 lines per detection class.

**Why no Flink:** the all-Rust runtime stack eliminates the JVM, GC pauses, and operational complexity of Flink while covering ~95% of detection expressiveness. The detail of this trade-off, including what Flink uniquely gives that Rust alternatives don't fully match, is in `07_Stack_Revised.md` Section 2. The detection DSL compiles to whichever engine is appropriate per rule.

### 3.4 Routing

Every source can fan out to:

- Attest hot store (default).
- Customer's existing SIEM (Splunk HEC, Sentinel, Chronicle) — for migration coexistence.
- Customer's data lake (S3/GCS/ABFS as Iceberg).
- External SOAR or ticketing.

Routing rules are declarative and versioned in Git.

## 4. Storage plane

### 4.1 Hot tier — operational queries (seconds to minutes back)

**Component:** ClickHouse. Single hot OLAP engine for the entire platform; no Druid, no alternative.

- 7–30 days of hot, indexed events.
- Sub-second point queries; single-digit-second aggregate queries over 24h.
- Optimized for SOC console, real-time dashboards, agent investigation tool calls.
- Same engine reads the warm Iceberg tier (Section 4.2), so detection authors write one dialect of SQL across hot and warm.

### 4.2 Warm tier — historical detection and hunting (days to years)

**Component:** Apache Iceberg on S3/GCS/ABFS, queried by ClickHouse via its native Iceberg table function. **Locally we use MinIO** (S3-compatible) so the same code path runs identically on a developer laptop and in production.

- All events normalized to OCSF, Parquet-encoded with Iceberg manifests.
- Schema evolution without downtime.
- Customer-owned-bucket option (BYOC): the customer controls the encryption key, the bucket policy, and the retention.
- Time-travel queries via Iceberg snapshots.
- **No Trino at GA.** ClickHouse-over-Iceberg covers the SOC analytics workload at one fewer system to operate. If a specific customer requires Trino for federated workloads, we add it as an opt-in component in BYOC. (See `07_Stack_Revised.md` Section 3.)
- Iceberg's open format means the customer can leave Attest and keep querying with any compatible engine (Trino, DuckDB, Snowflake, Spark, BigQuery).

### 4.3 Cold tier — compliance retention (years)

**Component:** S3 Glacier Deep Archive (or equivalent), addressable by Iceberg manifests.

- 7-year retention default for regulated customers.
- Re-hydratable to warm in minutes for incident response.

### 4.4 Tiering policy

Tiering is **data-class-driven, not volume-driven.** The default policy:

- Identity, EDR, agent telemetry → hot for 14 days, warm for 1 year, cold for 7 years.
- Network/firewall → hot for 7 days, warm for 90 days, cold for compliance window.
- Debug/heartbeat → never persisted past 24 hours.

Customers customize per data class. The default is conservative and tuned for cost.

## 5. Detection plane

### 5.1 Portable detection DSL

This is the architectural heart of the platform.

A single declarative DSL — call it `HELIQL` (Attest Query Language) — compiles to three execution targets:

- **Stream:** RisingWave for SQL-shaped detections; Arroyo for stateful CEP; custom Rust service for the edge cases.
- **Batch:** ClickHouse SQL over Iceberg.
- **Federated:** push-down to external engines (Snowflake, BigQuery, Splunk, Sentinel) via their native query language.

DSL design principles:

- Declarative; security analyst readable.
- Sigma-compatible (auto-import, auto-export).
- OCSF-native field references.
- First-class temporal primitives (sequences, windows, persistence).
- First-class entity primitives (per-user, per-host, per-agent baselines).

A single rule:

```
detection: aws_console_login_from_anomalous_geolocation
description: |
  Detects AWS console login from a country the user has not
  authenticated from in the past 90 days.
applies_to: ocsf.authentication
where:
  - event.activity = "Logon"
  - event.cloud_provider = "aws"
  - event.outcome = "success"
entity: identity.user
condition:
  unique(event.source.country, window: 90d) > 0
  AND event.source.country NOT IN baseline(identity.user.source.country, 90d)
severity: medium
mitre: [T1078.004]
runtime: stream | batch | federated
```

The same rule runs in real time on the stream, on demand against 90-day Iceberg history, and as a federated push-down to a customer's existing data lake.

### 5.2 ML detections

A plug-in interface accepts:

- Attest-trained models (managed).
- Bring-your-own ONNX/PyTorch models (customer-managed).

Models are deployed as side-cars in the stream pipeline; results join the same alert pipeline as DSL rules. Drift monitoring is built in.

### 5.3 UEBA primitives

Per-entity baselines computed continuously: user, host, service account, AI agent. Anomaly scoring via lightweight statistical models (median absolute deviation, robust z-score) on the stream; richer models (Isolation Forest, autoencoder) in the batch tier.

### 5.4 Detection-as-code lifecycle

```
Author → Lint → Unit test → Backtest (90d Iceberg) → Shadow deploy
       → Measure (precision, recall, FP volume) → Promote → Monitor → Tune | Retire
```

Every step is gated. Backtesting is not optional. The Self-Improving Detection Mesh (Section 7) is the agentic version of this same lifecycle.

## 6. Agentic plane (VAS)

### 6.1 Specialist agents

Five roles plus a Coordinator. Each agent has a distinct execution path, tool catalog, and policy scope. The **Triager is explicitly hybrid** — a calibrated XGBoost classifier handles ~80% of alert volume in under 50ms, with an LLM escalation path for the novel/ambiguous tail. Full specification in `09_Agent_Harness.md` Section 4.1.

| Agent | Job | Execution path | Tools |
|---|---|---|---|
| **Coordinator** | Plan which specialists to invoke per case | Classifier (lightweight routing logic) | Specialist invocation, case-state read |
| **Triager** | First-pass disposition of every alert | **Hybrid** — XGBoost classifier primary (80% of volume, <50ms, SHAP attribution); LLM escalation (Claude Sonnet / Qwen 3 7B local) for novel or low-confidence cases | Hot-tier query, threat intel lookup, asset context, identity lookup |
| **Investigator** | Deep investigation for escalated cases | LLM (Claude Opus / Qwen 3 32B air-gapped), with extended thinking | Triager tools + warm-tier query, packet/payload decoder, sandbox detonation, code analyzer |
| **Hunter** | Hypothesis-driven proactive hunting | LLM (Claude Opus + extended thinking) | Federated query, threat intel, MITRE knowledge graph, custom rule executor |
| **Detection Engineer** | Authors, tests, tunes detections | LLM (Claude Opus, code-capable) | DSL compiler, backtest harness, coverage map, Git PR creator |
| **Responder** | Executes containment actions | LLM (Claude Sonnet, low temperature, constrained) — *all actions gated by policy engine + shadow check* | SOAR connector, IdP connector, EDR connector |

### 6.2 Why multi-agent (vs. one big agent)

Four reasons:

- **Right tool per job.** A monolithic agent forces one execution path; Attest agents have classifier, LLM, and hybrid paths chosen per role. Triager's classifier path handles ~80% of alert volume in milliseconds; LLMs run only where they earn it.
- **Cost.** Triager handles 80%+ of volume. The classifier path costs effectively nothing per case; LLM escalation costs ~$0.005 per case. Without specialization, all volume hits the most expensive tier.
- **Specialization.** Different prompts, different tool catalogs, different temperature settings, different evaluation criteria, different calibration models per case-class.
- **Governance.** Per-agent policy scope is far easier to audit and lock down than a monolithic agent. Responder's policy is much tighter than Hunter's by design.

### 6.3 Tool access via MCP

All tool access is **through MCP**, not through ad-hoc HTTP. This gives:

- Standardized tool schemas; auditable tool registry.
- Mediation through the policy engine: every MCP call is intercepted, authorized, logged.
- Composability with the broader MCP ecosystem (security-vendor MCP servers can be plugged in).

### 6.4 Attestation

Every agent action emits a signed **attestation envelope**. Three variants depending on the execution path that produced the verdict — `classifier`, `llm`, or `hybrid` — all sharing a common signed wrapper so downstream verification is uniform. The full schema, including the path-specific evidence blocks (SHAP values for classifier paths; tool-call traces and citations for LLM paths; both for hybrid), is specified in `09_Agent_Harness.md` Section 4.1.5.

Common wrapper (subset):

```json
{
  "envelope_version": "1.1",
  "agent_action_id": "...",
  "case_id": "...",
  "tenant_id": "...",
  "execution_path": "classifier" | "llm" | "hybrid",
  "agent": {
    "id": "triager-hybrid-v3.2",
    "role": "Triager",
    "policy_version": "v42"
  },
  "timing": { "started_at": "...", "ended_at": "..." },
  "final_verdict": {
    "label": "true_positive",
    "category": "credential_stuffing_aws",
    "confidence_calibrated": 0.87
  },
  "shadow_check": { "policy_id": "...", "result": "allowed" },
  "classifier_evidence": { /* present for classifier and hybrid paths */ },
  "llm_evidence":        { /* present for llm and hybrid paths */ },
  "escalation_reason":   "low_calibrated_confidence" /* hybrid only */,
  "signature": "ed25519:..."
}
```

These envelopes are stored in an append-only, tamper-evident log (e.g., Sigstore-style transparency log) for as long as the customer's audit retention requires. The time-travel debugger (§6.8) renders all three variants uniformly — SHAP attribution for classifier, reasoning trace for LLM, side-by-side for hybrid.

### 6.5 Confidence calibration

Both paths produce raw confidence scores that are not natively well-calibrated. XGBoost classifiers tend toward over-confident scores at the extremes; LLMs are notoriously poor at expressing their own uncertainty. **Both need post-hoc calibration against actual outcomes.**

- Every agent decision is logged with raw confidence + final outcome.
- A calibration layer (isotonic regression, per agent + per execution path + per case-class) maps raw confidence to **calibrated probability of correctness**. Separate models per path capture their distinct miscalibration patterns.
- Calibrated confidence is what the policy engine uses to gate autonomy thresholds.
- Brier score and Expected Calibration Error are tracked per agent per path and reported in the workbench. Customers can audit the calibration of every decision-making path the platform exposes — a differentiator no competitor publishes.

Full specification in `09_Agent_Harness.md` Section 4.5.

### 6.6 Shadow deterministic verification

For any **agent-initiated action above a configurable risk threshold**, a parallel deterministic check runs before execution:

- Containment action → check against a deterministic rule that the target is not on a "do not contain" list (CEO laptop, critical production server, etc.).
- Detection deployment → check that lint, backtest, and shadow metrics meet promotion criteria.
- Mass remediation → check rate limits, blast radius bounds.

If the deterministic check disagrees with the agent, the action is paused for human review. This is the safety net that lets a regulated CISO sleep at night.

### 6.7 Hallucination guardrails

- **Citation enforcement:** every claim in an investigation must cite a specific OCSF event ID. Uncited claims are auto-rejected by the Investigator's own validation step.
- **Evidence retrieval before reasoning:** agents are constrained to first retrieve, then reason. They cannot reason from training-data prior alone.
- **Cross-agent review:** for high-impact verdicts, a second specialist independently reviews the first's reasoning trace before the verdict is finalized.

### 6.8 Time-travel debugger

The workbench can replay any past agent decision step by step. The replay re-executes the same prompts against the same data (read-only) using the recorded model version, and shows divergences if any. This is critical for incident retrospectives, regulatory audits, and detection-engineer learning.

## 7. Self-Improving Detection Mesh (SIDM)

The **Detection Engineer agent** runs continuously, not on demand. Its loop:

1. Read MITRE ATT&CK coverage map.
2. Read recent threat intel (TTPs, IoCs, CVE chatter).
3. Read last 7 days of investigations: which true positives had no detection that fired? Which detections fired with low precision?
4. Generate proposed detections in HELIQL with rationale.
5. Backtest against last 90 days of Iceberg data.
6. Open a Git PR with: rule, rationale, backtest precision/recall, expected hits/week, FP estimate, MITRE mapping.
7. On human approval and merge, deploy in shadow mode for 7 days.
8. On meeting promotion thresholds, auto-promote to production.
9. Continuously monitor; propose retirement of low-yield detections.

The customer's detection engineering team **never disappears**. Their job changes from "writing rules" to "approving and shaping rules proposed by an agent," at 5–10× the throughput.

## 8. Agent-Aware Detection Fabric (AADF)

### 8.1 New telemetry classes

Native ingest of:

- **OpenTelemetry GenAI traces** — prompt, completion, model, token counts, tool calls, latencies. Standard semantic conventions.
- **MCP server/client logs** — every MCP call: server name, tool, arguments hash, result hash, agent caller, outcome.
- **A2A protocol traces** — peer agent identity, conversation IDs, delegated tasks, exchanged state.
- **Agent reasoning logs** — system prompts, intermediate chain-of-thought summaries (configurable; full or hashed).
- **Vector store access logs** — RAG retrievals, document IDs, similarity scores.

### 8.2 New entity model

`AIAgent` becomes a first-class entity in OCSF (we contribute the schema upstream). Attributes include `model`, `model_version`, `system_prompt_hash`, `allowed_tools`, `owner`, `deployment_environment`, `attestation_chain`. Agent identity is distinct from the underlying service account; Attest correlates the two automatically.

### 8.3 New behavioral baselines

Per-agent baselines:

- Tool-call distribution (which tools, how often).
- Target distribution (which APIs, which records).
- Output size distribution.
- Latency distribution.
- Inter-agent communication graph.

Anomalies on these baselines fire detections — even when individual actions look legitimate.

### 8.4 Pre-built detection content for OWASP LLM Top 10

Out-of-box detections for:

- **LLM01 — Prompt injection** (direct via prompt anomaly + indirect via document/email content).
- **LLM02 — Sensitive information disclosure** (output filter triggers, regex matches, PII in completions).
- **LLM06 — Excessive agency** (tool-call anomalies, privilege escalation chains).
- **LLM07 — System prompt leakage**.
- **LLM08 — Vector and embedding weaknesses** (RAG poisoning).
- **LLM09 — Misinformation** (cross-checking high-stakes outputs against grounded sources).
- **LLM10 — Unbounded consumption** (cost/loop anomalies).

These ship as Sigma-compatible detections so other SIEMs can adopt them; Attest is open about the content.

### 8.5 Cross-domain correlation

The killer feature: correlating agent telemetry with classical telemetry. Example:

- Agent receives email → email contains hidden instructions (LLM01 detector fires, low-medium severity).
- Agent makes anomalous tool call to billing API (LLM06 detector fires, medium severity).
- Service account underlying the agent generates network egress to unrecognized destination (classical detection, medium severity).
- Attest correlates all three into a single high-severity case attributed to a chained agent compromise.

No existing SIEM does this because none has the agent telemetry schema.

## 9. Control plane

- **Identity:** OIDC via Okta/Entra/Auth0. SCIM for user provisioning. SSO required for all enterprise deployments.
- **Multi-tenancy:** Tenant ID propagated through every event, every query, every agent action. Per-tenant encryption keys (BYOK via AWS KMS, GCP KMS, Azure Key Vault).
- **GitOps:** All configuration (detections, pipelines, policies, agent definitions) is Git-backed. Configuration changes flow through PR review, signed at build, immutable at runtime.
- **Infrastructure:**
  - **Railway** for MVP and early design partners — fast iteration, low ops, sufficient for pre-Series A.
  - **Terraform** modules for BYOC deployments on AWS (EKS), GCP (GKE), Azure (AKS). Same container artifacts deploy to either Railway or K8s.
  - **Helm** charts for self-hosted and air-gapped reference deployments.
- **Observability:** Prometheus + Grafana. Per-tenant SLO dashboards. Customer-facing status page.
- **Audit:** Append-only audit log of every config change, query, agent action, integration event. 7-year retention by default for regulated customers.

## 10. Deployment models

### 10.1 Railway-managed (MVP)

The default for early design partners. Attest-managed Railway projects host both control plane and data plane. Iceberg warm tier still lives on customer-owned S3 from day one (per §4.2), so customer data never co-mingles with Attest infrastructure beyond the hot tier. Sufficient for the first ~10 design partners pre-Series A.

### 10.2 SaaS (multi-tenant)

Multi-tenant control plane, single-tenant data plane (per-tenant compute namespace + per-tenant storage tier), Attest-managed cloud (AWS primary, GCP/Azure roadmap). Ships in GA v1.

### 10.3 Customer-cloud (BYOC)

Attest's control plane runs in Attest's cloud; the data plane runs in the customer's VPC on their EKS/GKE/AKS. The customer's data, including the warm Iceberg tier, never leaves their cloud. This is critical for healthcare, financial services, and defense. Same container artifacts as Railway and SaaS deployments.

### 10.4 Air-gapped

Reference architecture for fully air-gapped environments (defense, intelligence). Helm chart, offline open-weights model bundles (Qwen 3 served via vLLM), manual update process. Not a primary GTM motion in year 1, but the reference must exist for credibility with defense buyers.

## 11. Security and compliance posture

- SOC 2 Type II from launch.
- ISO 27001 within 12 months.
- HIPAA-eligible deployment configuration from launch.
- FedRAMP Moderate roadmap committed for year 2.
- Per-tenant BYOK (bring your own key) at GA.
- All data in transit: TLS 1.3 with mTLS for service-to-service.
- All data at rest: AES-256 with customer-controlled keys in BYOC.
- Penetration test by independent firm prior to GA, then annually.
- Bug bounty program from GA.
- Software bill of materials (SBOM) published per release.
- Sigstore signing of all release artifacts.

## 12. Performance budget

| Path | Target P50 | Target P99 |
|---|---|---|
| Edge collector → ingest backbone | 50 ms | 200 ms |
| Ingest → in-stream detection alert | 1 s | 5 s |
| Hot-tier query (24h, single entity) | 200 ms | 1 s |
| Warm-tier query (30d, aggregate) | 5 s | 30 s |
| Triage agent verdict | 8 s | 30 s |
| Investigator agent verdict | 30 s | 3 min |
| Detection backtest (90d, single rule) | 1 min | 5 min |

These are design targets. Every change must be benchmarked against them in CI.

## 13. Reference physical topology (Railway, single tenant, mid-market MVP)

- **Ingest:** Redpanda single broker (scale to 3-broker cluster for production), 8-core / 32GB.
- **Collectors:** customer-side, single Rust binary, sized to source.
- **Stream runtime:** RisingWave (single binary, Rust, persistent volume), 8-core / 32GB. Arroyo as separate service for stateful CEP, 4-core / 16GB.
- **Hot store:** ClickHouse, NVMe-backed Railway volume, 16-core / 64GB. Reads warm Iceberg via the same engine.
- **Warm store:** Iceberg on customer S3, ~$0.023/GB-month with intelligent tiering. **Local dev: MinIO** with the same paths.
- **Cold store:** Glacier Deep Archive, ~$0.001/GB-month.
- **Agentic plane:**
  - Rust orchestrator (handles all three execution paths in-process), 4-core / 16GB.
  - Python ML sidecar (XGBoost classifier inference, calibration, novelty detector, evaluation runner), 4-core / 16GB.
  - Inference router (Rust): Anthropic API primary, llama.cpp + Qwen 3 7B for local dev and Triager-only deployments.
- **Workbench:** Next.js on Vercel (or Railway); Rust REST API + Rust WebSocket gateway, 4-core / 16GB combined.

A typical mid-market design partner runs Attest on Railway for **$800–$1,500/month of platform cost** plus LLM API spend (~$2K–$5K/month at MVP volumes). See `07_Stack_Revised.md` Section 10 for the cost breakdown.

For BYOC and post-Series A SaaS deployments, the topology scales horizontally on the same container artifacts deployed to EKS/GKE/AKS.

## 14. Build vs. buy

| Component | Decision | Rationale |
|---|---|---|
| Streaming backbone | **Buy** (Redpanda) | Single binary, Kafka-compatible, no JVM. |
| Stream runtime | **Buy** (RisingWave + Arroyo, both Rust) | All-Rust runtime; no Flink, no JVM. |
| Hot OLAP | **Buy** (ClickHouse) | Reads Iceberg directly; one engine for hot + warm. |
| Warm storage | **Use open standard** (Iceberg) | Customer-portable; no lock-in. |
| Local object store | **Buy** (MinIO) | S3-compatible; same code path as production. |
| Edge collector | **Build** (Rust) | Differentiating; footprint matters. |
| Detection DSL & compiler | **Build** | Core IP. The portable detection promise. |
| **Triager classifier** | **Build** (XGBoost + ONNX in Rust orchestrator) | **Core IP. ~80% of alert volume runs through this.** |
| Agentic plane orchestrator | **Build** (Rust) | Core IP. The Verifiable Agentic SOC. |
| ML sidecar | **Build** (Python, thin) | Wraps pretrained models and trainers. |
| Workbench frontend | **Build** (Next.js + Rust APIs) | Core IP. UX is the moat for analysts. |
| WebSocket gateway | **Build** (Rust) | Performance-critical. |
| LLM inference | **Buy multi-provider** (Anthropic + vLLM + llama.cpp) | Frontier capability; per-role routing. |
| MCP gateway | **Build** (Rust thin layer) | Need policy enforcement integrated. |
| Prompt-injection classifier | **Buy** (pretrained DeBERTa) | Mature, cheap. |
| Embeddings | **Buy** (BGE family pretrained) | Mature, cheap. |
| SOAR | **Integrate** (Torq/Tines/XSOAR) | Don't fight the SOAR vendors. |
| GitOps + IaC | **Buy** (Terraform for BYOC; Railway native for MVP) | Standard tooling. |

## 15. Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| LLM cost per case spirals | Margin death | Tiered agent stack (cheap Triager); aggressive caching; bring-your-own-inference for cost-sensitive customers |
| Hallucinated agent verdicts cause incident | Trust collapse | Citation enforcement, shadow checks, calibrated confidence, mandatory human-in-the-loop above thresholds |
| Detection DSL fragments adoption (one more thing to learn) | Slow uptake | Sigma compatibility from day 1; auto-translate from common formats; NL→DSL via Detection Engineer agent |
| Customers fear data sovereignty | Lost regulated deals | BYOC option from GA; air-gapped reference architecture |
| Big incumbents copy the agentic playbook | Eroded differentiation | Open-source the AADF detection content (LLM Top 10) to seed the category; keep moat in VAS governance and SIDM closed-loop |
| Agent supply-chain compromise (Attest's own agents poisoned) | Existential | Sigstore-signed agent definitions; reproducible builds; periodic red-team of Attest's own agents |

## 16. Open architectural questions

Resolved since this document was first drafted (decisions captured in `07_Stack_Revised.md`, `08_Datasets_and_ML.md`, `09_Agent_Harness.md`):

- ~~Is the warm tier ClickHouse or Iceberg+Trino at GA?~~ → **ClickHouse-over-Iceberg** at MVP and GA v1; Trino as opt-in for customers with specific federated needs.
- ~~Do we use Flink or RisingWave for stream runtime?~~ → **All-Rust: RisingWave + Arroyo + custom Rust** for the edge cases. No Flink.
- ~~Is the Triager an LLM agent?~~ → **Hybrid by design**: XGBoost classifier primary (~80% of volume), LLM escalation tail (~20%).

Still open, to be resolved with design partners:

1. Do we ship our own MCP gateway or rely on a community/standardized one?
2. Is HELIQL strictly declarative, or do we allow Python escape hatches (Panther-style)?
3. How do we expose the calibrated-confidence layer to customers who want to apply their own risk weighting?
4. What is the UX for showing reasoning traces to a non-engineer auditor?
5. How aggressively do we promote per-customer Triager classifier fine-tunes — automatic monthly or human-gated?
6. How much of the existing detection content (Sigma, Splunk SPL, KQL) must we auto-translate vs. ask customers to rewrite?
