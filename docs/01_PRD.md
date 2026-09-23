# 01 — Product Requirements Document

**Product:** Attest
**Status:** Draft v0.1
**Last updated:** April 2026

> **Design document.** Describes the target architecture. For what is implemented today, see the Status table in the root README.

---

## 1. Vision

Attest is the first security operations platform built from the ground up for a world in which AI agents are the dominant attackers and the dominant defenders. It is streaming-first, composable, AI-native, and — uniquely — verifiable. It gives security teams a substrate where every byte is processed in motion, every detection is portable across stream and history, every AI agent is governed and auditable, and every AI-driven decision can be replayed, attested, and trusted.

## 2. Mission

To eliminate the trade-off between machine-speed defense and human-grade accountability.

## 3. Problem statement

Security operations are stuck between two failing models:

- **Legacy SIEMs** (Splunk, QRadar, ArcSight) are storage-coupled, indexing-coupled, and human-paced. They cost too much, alert too noisily, and detect too slowly for the modern threat landscape.
- **First-wave AI SOCs** (Microsoft Copilot, Charlotte AI, Prophet, AiStrike) are real progress, but they layer agentic AI on top of opaque architectures, ask buyers to trust the agent's verdict, and don't natively detect threats that *originate from AI agents themselves*.

Meanwhile, the threat landscape has changed underneath the industry:

- Mean time to exploit collapsed from 23 days (2025) to 1.6 days (2026).
- Ransomware kill chains compressed to under 25 minutes via agentic offensive frameworks.
- Identity weaknesses are implicated in nearly 90% of incidents, and AI agents are now identities.
- 99% of organizations report alert fatigue is preventing them from finding real threats.
- 80% of stored security data is never queried, yet teams pay to keep it hot.
- New attack classes — prompt injection, indirect prompt attacks, A2A session smuggling, MCP exploitation, memory poisoning, agent tool misuse — bypass every existing SIEM rule because the actions look legitimate.

There is no platform that treats AI agents as both a detected entity *and* a defending entity, and no platform that makes the defending agents auditable enough to satisfy a regulated CISO. Attest is built specifically to occupy that gap.

## 4. Target market

### 4.1 Primary ICP (Year 1)

- **Mid-market to lower-enterprise** (1,000–10,000 employees, $200M–$5B revenue) in sectors with both heavy regulatory pressure and aggressive AI adoption: **financial services, healthcare/pharma, insurance, defense supply chain, regulated SaaS**.
- **Existing SIEM cost pain:** annual SIEM bill of $500K–$5M and growing 25–40% per year.
- **Mature security org:** dedicated SOC, detection engineering function, willingness to write rules.
- **Active AI program:** at least one production AI agent or copilot internally, ideally several. They feel the agentic threat surface viscerally.

### 4.2 Secondary ICP (Year 2)

- **MSSPs and MDR providers** building agentic SOC offerings on top of Attest as the substrate.
- **Regulated enterprise** (Fortune 1000 banks, health systems, government).

### 4.3 Anti-ICP

- Pure SMB without security staff (no detection engineering capacity to leverage Attest power).
- Organizations that want a fully-managed black-box service (better fit for an MDR partner using Attest).
- Single-cloud Microsoft shops happy with Sentinel + Copilot (Attest's value is in heterogeneous, multi-cloud, AI-heavy environments).

## 5. Personas

### 5.1 Riya — VP of Detection Engineering at a $2B fintech

- **Pain:** Her team of six writes detections in Splunk SPL. Coverage gaps are everywhere. Tuning is reactive. She's been told to evaluate three replacement platforms in 90 days.
- **Wins with Attest:** Self-Improving Detection Mesh proposes detections she didn't think of, backtested against her own data, with measurable coverage uplift against MITRE ATT&CK.

### 5.2 Marco — CISO at a regional health system

- **Pain:** Auditors are asking how AI is used in his SOC. He's piloting Copilot but can't answer "show me why the AI took this action" in a way that satisfies HIPAA. He won't let an agent take destructive action without justification he can defend.
- **Wins with Attest:** Verifiable Agentic SOC produces signed reasoning traces, calibrated confidence, and a replayable timeline of every agent decision. Policy-as-code prevents agents from acting outside scope.

### 5.3 Diego — SOC Lead at a defense supplier

- **Pain:** Drowning in alerts. Tier-1 analysts churn every 14 months. Wants 24×7 autonomous triage but can't trust an LLM with his queue.
- **Wins with Attest:** Triage agents handle 80% of alert volume autonomously with calibrated confidence; ambiguous cases escalate with full context. Diego's team supervises rather than triages.

### 5.4 Priya — Head of AI Platform at a pharma giant

- **Pain:** 40+ internal AI agents now have access to her data lake. She has zero detective capability for prompt injection, tool misuse, or rogue agent behavior. Her CISO is asking her to "monitor the AI."
- **Wins with Attest:** The Agent-Aware Detection Fabric ingests her OTEL GenAI traces, MCP logs, and agent reasoning logs alongside identity and network telemetry. She finally has a single answer to "what are the AI agents doing right now."

### 5.5 Sam — Senior Threat Hunter at an MSSP

- **Pain:** Hunting across 50+ tenant SIEMs, each with different schemas. Hypothesis-driven hunts take days.
- **Wins with Attest:** Federated detection executes hypotheses across all tenant data without re-ingestion. The Hunter Agent surfaces candidate hypotheses based on threat intel and tenant context.

## 6. Key use cases

### UC-1: Stream-first ingest with intentional shaping

Riya's team ingests 12 TB/day from CloudTrail, EDR, Okta, Zscaler, K8s, and 80 SaaS apps. Attest shapes, normalizes (OCSF), enriches, and routes data at the edge. Low-value telemetry (debug logs, redundant heartbeats) is dropped or sent to cold storage. High-signal data flows to in-stream detection in <5 seconds. Routing splits a single source to multiple destinations (Attest hot store + customer's existing Splunk during migration + Iceberg on customer's S3 for long retention).

### UC-2: Portable detection across time horizons

Riya writes a detection in Attest DSL once. The same logic runs:
- **In-stream** for real-time threats (suspicious process + network egress within 5s).
- **Historical** as a 30-day retro hunt over Iceberg.
- **Federated** as push-down to her existing Snowflake without ingesting that data.

She doesn't rewrite the rule three times.

### UC-3: Autonomous triage with verifiable verdict

A potential credential-stuffing alert hits. The Triage Agent:
1. Pulls related identity, endpoint, and geolocation context.
2. Checks threat intel feeds and historical false-positive patterns.
3. Decodes any obfuscated payloads it encounters.
4. Writes a structured verdict with calibrated confidence (e.g., "true positive, 0.87").
5. Produces a signed reasoning trace listing every data source, tool call, intermediate hypothesis, and the model version used.
6. Auto-closes if confidence > policy threshold and verdict is benign; escalates otherwise.

Marco's auditor opens the case six months later and replays the entire decision tree.

### UC-4: Agent-aware detection of a rogue internal AI

Priya's customer-support AI agent receives an indirect prompt injection via an email it processed. It begins exfiltrating PII through a tool call. Attest detects:
- Anomalous tool-invocation pattern vs. learned baseline (volume, sequence, target).
- Correlates against the agent's reasoning trace (presence of injected instructions).
- Cross-references network egress to an unrecognized destination.
- Issues a high-confidence detection scoped to the agent identity, not just the underlying service account.

The Responder Agent revokes the agent's tokens within seconds, after a deterministic shadow check confirms the action is safe (won't disable critical services).

### UC-5: Closed-loop detection improvement

The Detection Engineering Agent reviews the past 7 days of investigations. It notices that 12% of true-positive credential-theft cases involved a particular OAuth flow not currently covered by any detection. It drafts a new Sigma-compatible rule, backtests it across 90 days of Iceberg data (catches 9 historical hits, 0 false positives), opens a PR. Riya reviews, approves, merges. The rule deploys in shadow mode for 7 days, gets auto-promoted to production after meeting precision/recall thresholds.

### UC-6: Federated cross-tenant hunt (MSSP)

Sam's MSSP serves 30 tenants. A new IOC appears in threat intel. The Hunter Agent runs a federated query across all 30 tenants' Attest deployments without exfiltrating data, returns matches scoped per tenant, and creates pre-investigated cases for each affected SOC.

## 7. Functional requirements (high level)

### 7.1 Data plane

- **R-DP-1:** Stream-first ingest at sustained 1M events/sec per tenant, with horizontal scaling to 10M.
- **R-DP-2:** Native collectors and connectors for top 50 sources (AWS, Azure, GCP, Okta, Zscaler, CrowdStrike, SentinelOne, Microsoft 365, Workday, etc.) plus generic Syslog, OTEL, OCSF, CEF, JSON.
- **R-DP-3:** Edge data shaping: filter, aggregate, redact, route, enrich (geo, IAM mapping, asset criticality) before any billing meter.
- **R-DP-4:** OCSF normalization out of the box for all native connectors.
- **R-DP-5:** Multi-destination routing: Attest hot store + arbitrary external sinks (Splunk, Sentinel, Iceberg, S3, BigQuery, Snowflake) configurable per source.
- **R-DP-6:** Schema evolution without breaking detections.
- **R-DP-7:** Replay from durable buffer for at least 7 days.

### 7.2 Storage plane

- **R-ST-1:** Three-tier storage: hot (sub-second query, ClickHouse), warm (Iceberg, queried via ClickHouse Iceberg integration), cold (S3 Glacier or equivalent).
- **R-ST-2:** Customer-owned-bucket option (BYOC) for warm and cold tiers.
- **R-ST-3:** Open table format (Iceberg) with OCSF schema; data remains queryable by any compatible engine if customer leaves.
- **R-ST-4:** Retention policy as code, per data class, with audit trail.

### 7.3 Detection plane

- **R-DT-1:** Single detection DSL that compiles to stream (RisingWave / Arroyo), batch (ClickHouse over Iceberg), federated (push-down to external engines).
- **R-DT-2:** Sigma rule import; OCSF-aware translation.
- **R-DT-3:** Detection-as-code via Git, with CI/CD pipeline (lint, unit test, backtest, shadow deploy, promote).
- **R-DT-4:** Native support for stateful streaming detections (windowed aggregations, joins across streams, sequences).
- **R-DT-5:** UEBA primitives: per-entity baselines (user, host, agent, service account), anomaly scoring.
- **R-DT-6:** ML detection plug-in: bring your own model (PyTorch, ONNX) or use Attest-trained models.

### 7.4 Agent-Aware Detection Fabric (AADF)

- **R-AADF-1:** Native ingestion of OpenTelemetry GenAI traces, MCP server/client logs, A2A protocol traces.
- **R-AADF-2:** First-class entity model: AI Agent (with attributes: model version, prompt template hash, allowed tools, owner, deployment).
- **R-AADF-3:** Behavioral profiles per agent: baseline tool-call distribution, target distribution, latency, output size.
- **R-AADF-4:** Pre-built detection content for OWASP LLM Top 10 (LLM01–LLM10), with focus on LLM01 prompt injection, LLM02 sensitive information disclosure, LLM06 excessive agency, LLM07 system prompt leakage.
- **R-AADF-5:** Correlation primitives that join agent telemetry with classical telemetry (the agent identity → the service account it acts as → the network flows it produces).
- **R-AADF-6:** Detection of inter-agent attacks (A2A session smuggling, agent supply-chain compromise via MCP).

### 7.5 Verifiable Agentic SOC (VAS)

- **R-VAS-1:** Multi-agent architecture with at least five specialist roles: **Triager**, **Investigator**, **Hunter**, **Detection Engineer**, **Responder**.
- **R-VAS-2:** Every agent action emits an attestation: signed reasoning trace, data sources accessed, tool calls made, model version, prompt template version, intermediate hypotheses, final verdict, calibrated confidence.
- **R-VAS-3:** Confidence is calibrated against historical outcomes (Brier score, ECE), not raw LLM self-report.
- **R-VAS-4:** Shadow deterministic verification: any agent-initiated action above a configurable risk threshold is blocked until a parallel deterministic policy check confirms it.
- **R-VAS-5:** Time-travel debugger: any past agent decision can be replayed step-by-step in the workbench, including alternative branches.
- **R-VAS-6:** Policy-as-code governance: an external (non-LLM) policy engine determines what tools, data, and actions each agent can use. Agents cannot modify their own policy.
- **R-VAS-7:** Human-in-the-loop escalation with structured handoff (full context, recommended actions, confidence) — never raw alerts.
- **R-VAS-8:** Hallucination guardrails: every agent claim grounded in cited evidence; ungrounded claims rejected by the Investigator's own validation step.

### 7.6 Self-Improving Detection Mesh (SIDM)

- **R-SIDM-1:** Detection Engineering Agent that proposes new detections from MITRE ATT&CK coverage gaps, threat intel feeds, and recent incident patterns.
- **R-SIDM-2:** Auto-backtest against last 90 days of Iceberg data with precision/recall reporting.
- **R-SIDM-3:** Auto-PR via Git with detection rationale, expected hits, false-positive estimate.
- **R-SIDM-4:** Shadow deploy → measure → promote pipeline.
- **R-SIDM-5:** Continuous tuning: detect signal drift, propose threshold updates, propose retirement of low-yield detections.
- **R-SIDM-6:** Detection coverage map updated continuously, visualized vs. MITRE ATT&CK.

### 7.7 Workbench (UI/UX)

- **R-UI-1:** Unified case workbench: case management, hunting, response, investigation, detection authoring.
- **R-UI-2:** Natural-language query bar that compiles to detection DSL.
- **R-UI-3:** Reasoning-trace viewer: visual timeline of every agent action with expandable evidence.
- **R-UI-4:** Detection coverage heatmap (MITRE ATT&CK).
- **R-UI-5:** Agent governance console: per-agent policy, audit log, permission scopes.

### 7.8 Integration & ecosystem

- **R-INT-1:** Open APIs (REST, gRPC, GraphQL).
- **R-INT-2:** SOAR integration via Torq, Tines, Palo Alto XSOAR.
- **R-INT-3:** Identity integration (Okta, Entra, Ping).
- **R-INT-4:** Threat intel ingestion (MISP, Anomali, Recorded Future, custom feeds via STIX/TAXII).
- **R-INT-5:** MCP support: Attest exposes its own MCP server for analyst tooling and consumes external MCP servers as agent tools.

## 8. Non-functional requirements

| Category | Requirement |
|---|---|
| **Latency** | P99 in-stream detection latency ≤ 5 seconds from ingest |
| **Throughput** | Sustained 1M EPS per tenant; bursting to 5M; multi-tenant horizontal scale |
| **Availability** | 99.95% control plane, 99.9% data plane |
| **Recovery** | RPO 15 min, RTO 1 hour |
| **Security** | SOC 2 Type II, ISO 27001, FedRAMP-Moderate roadmap, HIPAA-eligible |
| **Privacy** | Customer-owned-bucket option; no customer data leaves customer cloud in BYOC mode |
| **Cost** | Target 50–70% TCO reduction vs. Splunk for equivalent retention and detection scope |
| **Open standards** | OCSF, OTEL, OTEL GenAI, MCP, Sigma, STIX/TAXII, Iceberg |
| **Auditability** | Every agent action signed; tamper-evident audit log; 7-year retention option |
| **Multi-tenancy** | Strict tenant isolation; per-tenant encryption keys (BYOK) |
| **Deployment** | SaaS, customer-cloud (AWS/GCP/Azure), air-gapped reference architecture |
| **Localization** | English at GA; Spanish, German, Japanese, Arabic by 12-month mark |
| **Accessibility** | WCAG 2.1 AA |

## 9. Success metrics

### 9.1 Customer outcomes (the only metrics that matter long-term)

- **MTTD reduction:** target median MTTD reduction of 60% vs. customer's prior SIEM at 90 days.
- **MTTR reduction:** target median MTTR reduction of 70% on agent-handled cases.
- **Alert volume to analyst inbox:** reduction of 80%+ via autonomous triage.
- **Detection coverage:** measurable MITRE ATT&CK coverage uplift in 90 days.
- **TCO:** 50–70% reduction in equivalent SIEM spend.
- **Auditor pass rate:** 100% of regulated customers pass SIEM-related audit findings on first attempt.

### 9.2 Product metrics

- Time-to-first-detection from signup: ≤ 1 hour.
- Triage agent autonomous closure rate (no human intervention) on benign cases: ≥ 80%.
- False-positive rate on agent autonomous closures: ≤ 0.1%.
- Self-Improving Detection Mesh net-new detections proposed per week per customer: ≥ 5.

## 10. Out of scope (for v1)

- Native EDR — Attest consumes EDR telemetry, doesn't compete with CrowdStrike/SentinelOne.
- Native vulnerability scanning — partners with Tenable, Qualys.
- Identity threat detection deep specialization — partners with SpyCloud, Push Security; Attest correlates their signals.
- Email security — partners with Abnormal, Proofpoint.
- Cloud workload protection — partners with Wiz, Lacework.
- Standalone DLP — partners with Cyberhaven.

The wedge is sharp on purpose. Win one architectural battle, then expand.

## 11. Open questions

1. What compliance evidence do regulated operators need for BYOC vs. SaaS deployment?
2. Which two SOAR platforms should be deeply integrated first?
3. How much existing detection content (Sigma, Elastic, Splunk SPL) should be auto-translated vs. rewritten?
4. Should the Detection Engineering Agent be autonomous-by-default or human-driven-by-default?
6. What governance certifications (NIST AI RMF, ISO/IEC 42001) do regulated buyers most want for the agentic layer?

## 12. Appendix: glossary

- **OCSF** — Open Cybersecurity Schema Framework. Vendor-neutral schema for security events.
- **OTEL GenAI** — OpenTelemetry semantic conventions for GenAI observability (prompts, completions, tool calls, model metadata).
- **MCP** — Model Context Protocol. Standardizes how LLMs interact with tools and context sources.
- **A2A** — Agent-to-agent protocol. Enables peer agents to delegate and coordinate tasks.
- **Sigma** — Open detection rule format, vendor-neutral.
- **Iceberg** — Open table format for analytic datasets with schema evolution and time travel.
- **OWASP LLM Top 10** — The standard taxonomy of vulnerabilities in LLM-powered systems (LLM01 prompt injection, …, LLM10 unbounded consumption).
- **MTTD / MTTR** — Mean time to detect / respond.
- **UEBA** — User and entity behavior analytics.
