# 03 — Architecture Diagrams

**Product:** Attest
**Document type:** Visual architecture reference — all Mermaid diagrams for the platform.
Companion to `02_Architecture.md`. Diagrams are organized from broadest to most detailed.

> **Design document.** Describes the target architecture. For what is implemented today, see the Status table in the root README.

---

## 1. C4 Level 1 — System Context

Who uses Attest, and what external systems does it touch.

```mermaid
C4Context
  title Attest — System Context

  Person(analyst, "SOC Analyst", "Triages alerts, investigates incidents, hunts threats via the workbench")
  Person(deteng, "Detection Engineer", "Authors, backtests, and deploys detection rules")
  Person(ciso, "CISO / Auditor", "Reviews agent decisions, calibration metrics, audit trail")

  System(attest, "Attest Platform", "Streaming-first, Verifiable Agentic SOC — ingests security telemetry, detects threats, and runs governed AI agents to triage and investigate")

  System_Ext(aws, "AWS (CloudTrail, GuardDuty, S3)", "Cloud control-plane telemetry")
  System_Ext(okta, "Okta / Entra ID", "Identity and authentication telemetry")
  System_Ext(edr, "EDR (CrowdStrike, SentinelOne)", "Endpoint telemetry")
  System_Ext(siem, "Existing SIEM (Splunk, Sentinel, Chronicle)", "Migration coexistence — Attest routes events to customer SIEM in parallel")
  System_Ext(soar, "SOAR (Tines, Torq, XSOAR)", "Attest Responder agent triggers containment playbooks")
  System_Ext(threat, "Threat Intel Feeds (MISP, VirusTotal, MITRE ATT&CK)", "Enrichment for triage and hunting")
  System_Ext(llm, "LLM Providers (Anthropic, vLLM / Qwen local)", "LLM inference for Investigator, Hunter, Detection Engineer, Responder agents")
  System_Ext(lake, "Customer Data Lake (S3 / GCS / ABFS)", "BYOC warm Iceberg tier — customer-owned, customer-encrypted")
  System_Ext(git, "Git (GitHub / GitLab)", "GitOps — detection rules, agent definitions, pipeline configs versioned here")

  Rel(analyst, attest, "Triages alerts, runs investigations, reviews agent verdicts", "HTTPS")
  Rel(deteng, attest, "Authors HELIQL rules, reviews SIDM proposals, promotes detections", "HTTPS / Git")
  Rel(ciso, attest, "Audits agent attestation log, reviews calibration, exports compliance reports", "HTTPS")

  Rel(aws, attest, "CloudTrail JSON / OTEL events", "HTTPS / Kafka")
  Rel(okta, attest, "Authentication / MFA events", "HTTPS / Kafka")
  Rel(edr, attest, "Endpoint telemetry (OCSF class 1001)", "HTTPS / Kafka")

  Rel(attest, siem, "Fan-out to existing SIEM (migration coexistence)", "Splunk HEC / Sentinel / Chronicle")
  Rel(attest, soar, "Responder agent triggers playbooks", "REST / Webhook")
  Rel(attest, threat, "Enriches alerts with intel lookups", "REST")
  Rel(attest, llm, "Agent escalation — Investigator, Hunter, Detection Eng, Responder", "HTTPS API")
  Rel(attest, lake, "Writes warm Iceberg tier (BYOC)", "S3 API")
  Rel(attest, git, "SIDM opens detection PRs; config pull on startup", "Git")
```

---

## 2. C4 Level 2 — Container Diagram (Six Planes)

The six architectural planes and the containers within each.

```mermaid
C4Container
  title Attest — Container Diagram (Six Planes)

  Person(analyst, "SOC Analyst")
  Person(deteng, "Detection Engineer")

  Boundary(workbench_plane, "Workbench Plane") {
    Container(ui, "SOC Workbench", "Next.js 15, React", "Alert queue, case investigation, time-travel debugger, detection coverage map")
    Container(api, "Control-Plane API", "Rust, Axum", "REST API — hot/warm queries, case CRUD, agent status, audit log")
    Container(ws, "WebSocket Gateway", "Rust, Tokio", "Real-time alert push to workbench")
  }

  Boundary(agentic_plane, "Agentic Plane (VAS)") {
    Container(orchestrator, "Orchestrator", "Rust, Axum :4300", "Hybrid triage loop — classifier path + LLM escalation; routes per agent role")
    Container(policy, "Policy Engine", "Rust (in-process)", "Hard-coded per-role authorization: Allow / Deny / Escalate")
    Container(mcp, "MCP Gateway", "Rust, Axum :4242", "Intercepts every tool call — logs, authorizes, dispatches")
    Container(attest_log, "Attestation Log", "Rust, ndjson", "Append-only Ed25519-signed evidence envelopes")
    Container(ml_sidecar, "ML Sidecar", "Python, FastAPI :5001", "Isotonic calibration; novelty parameters; XGBoost training harness")
    Container(llm_router, "Inference Router", "Rust", "Routes LLM requests: Anthropic API (prod) / vLLM + Qwen (local/air-gap)")
  }

  Boundary(detection_plane, "Detection Plane") {
    Container(heliql, "HELIQL Compiler", "Rust", "Parses detection DSL, compiles to RisingWave SQL / ClickHouse SQL / Arroyo SQL")
    Container(det_runtime, "Detection Runtime", "Rust", "Deploys HELIQL rules, polls materialized views, emits alerts to Redpanda")
    Container(sigma, "Sigma Importer", "Rust", "Converts Sigma YAML → HELIQL AST")
  }

  Boundary(storage_plane, "Storage Plane") {
    ContainerDb(clickhouse, "ClickHouse", "ClickHouse", "Hot OLAP — 7-30d indexed events; also queries warm Iceberg tier")
    ContainerDb(iceberg, "Iceberg / MinIO", "Apache Iceberg + MinIO (dev) / S3 (prod)", "Warm tier — years of OCSF Parquet, customer-owned BYOC")
    ContainerDb(glacier, "S3 Glacier", "AWS Glacier Deep Archive", "Cold tier — 7-year compliance retention")
  }

  Boundary(streaming_plane, "Streaming Plane") {
    Container(collector, "Edge Collector", "Rust, Axum :4000", "Ingests CloudTrail / Okta / EDR JSON, normalizes to OCSF 1.3, produces to Redpanda")
    ContainerDb(redpanda, "Redpanda", "Redpanda / Kafka-compatible :9092", "Event backbone — one topic per source class, 7-day replay buffer")
    Container(risingwave, "RisingWave", "RisingWave :4566", "Streaming SQL — materialized views for baselines, hot-tier detection, entity profiles")
    Container(arroyo, "Arroyo", "Arroyo (Rust)", "Stateful CEP — complex n-event sequence patterns exceeding RisingWave's expressiveness")
    Container(iceberg_writer, "Iceberg Writer", "Rust (attest-storage-iceberg)", "Kafka consumer → Arrow/Parquet batch → MinIO/S3 Iceberg")
  }

  Boundary(control_plane_layer, "Control Plane") {
    Container(idp, "Identity (OIDC)", "Okta / Entra / Auth0", "SSO, SCIM provisioning, per-tenant RBAC")
    Container(gitops, "GitOps", "Git + CI", "Detection rules, agent definitions, pipeline configs — all version-controlled")
    Container(infra, "Infrastructure", "Railway (MVP) / Terraform + EKS/GKE/AKS (BYOC)", "Container orchestration, secrets, networking")
  }

  Rel(analyst, ui, "Uses", "HTTPS")
  Rel(deteng, ui, "Manages detections", "HTTPS")
  Rel(ui, api, "API calls", "HTTPS/JSON")
  Rel(ui, ws, "Live alert stream", "WebSocket")

  Rel(api, clickhouse, "Hot-tier queries", "ClickHouse HTTP :8123")
  Rel(api, orchestrator, "POST /triage, GET /agent", "HTTP")

  Rel(collector, redpanda, "Produces OCSF events", "Kafka producer")
  Rel(redpanda, risingwave, "Kafka source", "Kafka consumer")
  Rel(redpanda, iceberg_writer, "Kafka source", "Kafka consumer")
  Rel(redpanda, arroyo, "Kafka source", "Kafka consumer")
  Rel(redpanda, det_runtime, "alerts topic → API", "Kafka consumer")

  Rel(risingwave, det_runtime, "Polls det_* views", "PostgreSQL")
  Rel(iceberg_writer, iceberg, "Writes Parquet", "S3 API")
  Rel(clickhouse, iceberg, "Queries via Iceberg table function", "S3 API")

  Rel(orchestrator, policy, "authorize(role, tool, ctx)", "in-process")
  Rel(orchestrator, mcp, "Tool calls", "HTTP")
  Rel(orchestrator, attest_log, "append(envelope)", "file I/O")
  Rel(orchestrator, ml_sidecar, "POST /calibrate", "HTTP")
  Rel(orchestrator, llm_router, "LLM escalation", "HTTP")

  Rel(mcp, clickhouse, "query_hot_tier", "ClickHouse HTTP")
  Rel(mcp, risingwave, "get_user_baseline", "PostgreSQL")

  Rel(heliql, risingwave, "CREATE MATERIALIZED VIEW", "PostgreSQL DDL")
  Rel(heliql, clickhouse, "Batch detection SQL", "ClickHouse HTTP")
  Rel(det_runtime, redpanda, "Produces alerts", "Kafka producer")
```

---

## 3. Data Flow — Event Journey (Source → Alert)

How a raw cloud event becomes a fired detection alert.

```mermaid
flowchart TD
    subgraph Sources["External Sources"]
        CT["☁️ AWS CloudTrail"]
        OK["🔐 Okta / Entra ID"]
        EDR["💻 CrowdStrike / SentinelOne"]
    end

    subgraph StreamingPlane["Streaming Plane"]
        COL["attest-collector\nOCSF 1.3 normalizer\n:4000"]
        RP["Redpanda\ntopic: cloudtrail / okta / edr\n:9092"]

        subgraph StreamRuntime["Stream Runtime"]
            RW["RisingWave\nMaterialized views\nEntity baselines"]
            subgraph ArroyoSvc["Arroyo :5115"]
                ARR_ETL["cloudtrail_to_parquet\nKafka → Parquet → MinIO"]
                ARR_CEP["cep_sequence_detection\nLogin → S3 sequence"]
            end
        end

        IW["attest-storage-iceberg\nKafka → Parquet → MinIO\n(parallel — migration)"]
    end

    subgraph StoragePlane["Storage Plane"]
        MINIO["MinIO / S3\nIceberg Parquet\n(warm tier)"]
        CH["ClickHouse\nHot OLAP\n7-30d indexed\n:8123"]
    end

    subgraph DetectionPlane["Detection Plane"]
        HELIQL["HELIQL Compiler\n.heliql rules → SQL DDL"]
        DR["attest-detection-runtime\nPolls det_* views every 2s"]
        ALERTS["Redpanda\ntopic: alerts"]
    end

    subgraph AgenticPlane["Agentic Plane"]
        ORCH["Orchestrator\nHybrid Triage Loop\n:4300"]
        MCP["MCP Gateway\n:4242"]
    end

    subgraph WorkbenchPlane["Workbench Plane"]
        API["Control-Plane API\n:8080"]
        UI["SOC Workbench\nNext.js :3000"]
    end

    CT -->|"POST /ingest\nCloudTrail JSON"| COL
    OK -->|"POST /ingest\nOkta events"| COL
    EDR -->|"POST /ingest\nEndpoint events"| COL

    COL -->|"OCSF FlatEvent\nKafka produce"| RP

    RP -->|"Kafka source"| RW
    RP -->|"Kafka source"| ARR_ETL
    RP -->|"Kafka source"| ARR_CEP
    RP -->|"Kafka source"| IW

    IW -->|"Parquet files\nS3 API"| MINIO
    ARR_ETL -->|"Parquet files\nS3 API\n(arroyo/ prefix)"| MINIO
    ARR_CEP -->|"Produce alert JSON\n(sequence fired)"| ALERTS

    HELIQL -->|"CREATE MATERIALIZED VIEW\ndet_*"| RW
    DR -->|"SELECT * FROM det_*\nevery 2s"| RW
    DR -->|"Produce alert JSON"| ALERTS

    ALERTS -->|"Kafka consume"| ORCH

    RW -->|"entity_baselines\nrecent_events"| API
    MINIO -->|"Iceberg table function"| CH
    CH -->|"Hot + warm queries"| API
    API -->|"REST / WebSocket"| UI

    ORCH -->|"Tool calls\nauthorized"| MCP
    MCP -->|"query_hot_tier"| CH
    MCP -->|"get_user_baseline"| RW

    style Sources fill:#1a1a2e,stroke:#e94560,color:#fff
    style StreamingPlane fill:#16213e,stroke:#0f3460,color:#fff
    style StoragePlane fill:#0f3460,stroke:#533483,color:#fff
    style DetectionPlane fill:#533483,stroke:#e94560,color:#fff
    style AgenticPlane fill:#1a1a2e,stroke:#e94560,color:#fff
    style WorkbenchPlane fill:#16213e,stroke:#0f3460,color:#fff
```

---

## 4. Agentic Plane — Component Diagram

How the five specialist agents, policy engine, MCP gateway, and attestation log are wired together.

```mermaid
flowchart TD
    ALERT["🚨 Incoming Alert\n(from alerts Redpanda topic)"]

    COORD["Coordinator\n(lightweight routing classifier)"]

    subgraph Specialists["Specialist Agents"]
        direction TB
        TRIAGER["Triager\nHybrid path\nXGBoost → LLM escalation"]
        INVEST["Investigator\nClaude Opus / Qwen 32B\nExtended thinking"]
        HUNTER["Hunter\nClaude Opus\nProactive hypothesis"]
        DETENG["Detection Engineer\nClaude Opus (code)\nSIDM loop"]
        RESP["Responder\nClaude Sonnet\nLow temp, constrained"]
    end

    subgraph Governance["Governance Layer"]
        direction LR
        POLICY["Policy Engine\nAuthorize per role+tool+ctx\nAllow / Deny / Escalate"]
        SHADOW["Shadow Check\nDeterministic verify\nbefore destructive actions"]
        ATTEST["Attestation Log\nEd25519-signed envelopes\nappend-only ndjson"]
        CALIB["Calibration\nIsotonic regression\nper role × case-class"]
    end

    subgraph Tools["Tool Layer (via MCP Gateway :4242)"]
        direction TB
        T1["query_hot_tier\n(ClickHouse)"]
        T2["query_warm_tier\n(Iceberg via ClickHouse)"]
        T3["get_user_baseline\n(RisingWave)"]
        T4["lookup_threat_intel\n(external feeds)"]
        T5["get_asset_context\n(CMDB)"]
        T6["sandbox_detonate\n(Phase 7+)"]
        T7["idp_revoke_session\n(Okta API — Responder only)"]
        T8["soar_trigger\n(Tines / Torq)"]
    end

    LLM["LLM Inference Router\nAnthropic API (prod)\nvLLM + Qwen (local)"]

    HUMAN["👤 Human Analyst\n(high-stakes review queue)"]

    ALERT --> COORD
    COORD --> TRIAGER
    COORD --> INVEST
    COORD --> HUNTER
    COORD --> DETENG
    COORD --> RESP

    TRIAGER -->|"raw confidence + features"| CALIB
    CALIB -->|"calibrated score"| TRIAGER
    TRIAGER -->|"low confidence / OOD"| LLM

    INVEST --> LLM
    HUNTER --> LLM
    DETENG --> LLM
    RESP --> LLM

    Specialists -->|"tool call request"| POLICY
    POLICY -->|"Allow"| T1
    POLICY -->|"Allow"| T2
    POLICY -->|"Allow"| T3
    POLICY -->|"Allow"| T4
    POLICY -->|"Allow"| T5
    POLICY -->|"Restrict to Investigator"| T6
    POLICY -->|"Restrict to Responder\n+ shadow check required"| T7
    POLICY -->|"Restrict to Responder"| T8
    POLICY -->|"Deny"| HUMAN

    RESP -->|"destructive action"| SHADOW
    SHADOW -->|"disagrees → pause"| HUMAN
    SHADOW -->|"agrees → execute"| T7

    Specialists -->|"emits signed envelope"| ATTEST

    style Governance fill:#1a1a2e,stroke:#e94560,color:#fff
    style Tools fill:#0f3460,stroke:#533483,color:#fff
    style Specialists fill:#16213e,stroke:#0f3460,color:#fff
```

---

## 5. Sequence — Hybrid Triage Flow

Step-by-step: an alert arrives at the orchestrator, runs the classifier path or escalates to LLM.

```mermaid
sequenceDiagram
    autonumber
    participant K as Redpanda<br/>(alerts topic)
    participant O as Orchestrator<br/>:4300
    participant FE as FeatureExtractor
    participant CL as OnnxClassifier<br/>(XGBoost tract-onnx)
    participant ND as NoveltyDetector<br/>(Mahalanobis)
    participant CAL as CalibrationSidecar<br/>Python :5001
    participant PE as PolicyEngine
    participant MCP as MCP Gateway<br/>:4242
    participant LLM as LLM Router<br/>(Anthropic / vLLM)
    participant AL as AttestationLog

    K->>O: POST /triage { alert_json, case_id, tenant_id }
    O->>FE: extract(ocsf_event) → AlertFeatures[8 f64]
    FE-->>O: { severity_score, source_class_id, ... }

    O->>CL: predict(features) → { raw_score, shap_values }
    CL-->>O: raw_score=0.92, shap=[+0.31, +0.18, ...]

    O->>ND: score(features) → novelty_score, is_ood
    ND-->>O: novelty_score=0.07, is_ood=false

    O->>CAL: POST /calibrate { raw_score, case_class }
    CAL-->>O: calibrated_score=0.89

    alt calibrated ≥ threshold AND NOT ood
        Note over O: Classifier Path (< 50ms)
        O->>PE: authorize(Triager, query_hot_tier, ctx)
        PE-->>O: Allow
        O->>MCP: invoke { tool_id: query_hot_tier, args }
        MCP-->>O: { events, threat_context }
        O->>AL: append(ClassifierEvidence envelope)
        AL-->>O: ok
        O-->>K: TriageVerdict { verdict: true_positive,<br/>confidence: 0.89, path: classifier, latency: 28ms }

    else low confidence OR ood
        Note over O: LLM Escalation Path (Phase 4b)
        O->>PE: authorize(Triager, query_hot_tier, ctx)
        PE-->>O: Allow
        O->>MCP: invoke { tool_id: query_hot_tier }
        MCP-->>O: { events }
        O->>LLM: invoke_agent(system_prompt, evidence, tools)
        loop Tool calls
            LLM->>O: tool_call { tool_id, args }
            O->>PE: authorize(Triager, tool_id, ctx)
            PE-->>O: Allow / Deny
            O->>MCP: invoke(tool_id, args)
            MCP-->>O: tool_result
            O->>LLM: tool_result
        end
        LLM-->>O: verdict + reasoning + citations
        O->>AL: append(HybridEvidence envelope)
        AL-->>O: ok
        O-->>K: TriageVerdict { verdict: needs_investigation,<br/>path: hybrid, llm_evidence: { ... } }
    end
```

---

## 6. Sequence — HELIQL Detection Lifecycle (SIDM Loop)

How a detection goes from idea to production via the Self-Improving Detection Mesh.

```mermaid
sequenceDiagram
    autonumber
    participant MITRE as MITRE ATT&CK<br/>Knowledge Graph
    participant TI as Threat Intel Feeds
    participant DETENG as Detection Engineer<br/>Agent (Claude Opus)
    participant HELIQL as HELIQL Compiler
    participant RW as RisingWave
    participant CH as ClickHouse (Iceberg)
    participant GIT as Git / PR
    participant HUMAN as Human Detection Eng
    participant PROD as Production Stream

    Note over DETENG: SIDM loop runs continuously
    DETENG->>MITRE: read coverage gaps (which ATT&CK techniques have no detection?)
    MITRE-->>DETENG: uncovered techniques
    DETENG->>TI: read last 7d TTPs, IoCs, CVE chatter
    TI-->>DETENG: active threat patterns

    DETENG->>DETENG: generate proposed HELIQL rule + rationale

    DETENG->>HELIQL: compile(detection_rule)
    HELIQL-->>DETENG: compiled SQL (RisingWave DDL + ClickHouse batch)

    Note over DETENG,CH: Backtest against 90d of warm tier
    DETENG->>CH: run backtest SQL on last 90d Iceberg data
    CH-->>DETENG: { hits: 47, precision: 0.91, recall: 0.88, fp_estimate: 4.2/week }

    DETENG->>GIT: open PR { rule.heliql, rationale.md,<br/>backtest_results.json, mitre_mapping }

    HUMAN->>GIT: review + approve PR
    GIT-->>DETENG: merged

    Note over DETENG,RW: Shadow deploy — 7 days
    DETENG->>HELIQL: compile → shadow DDL
    HELIQL->>RW: CREATE MATERIALIZED VIEW det_shadow_*
    RW-->>DETENG: shadow alerts (not sent to analysts)

    DETENG->>DETENG: measure shadow precision/recall vs. thresholds

    alt Meets promotion thresholds
        DETENG->>HELIQL: compile → production DDL
        HELIQL->>RW: CREATE MATERIALIZED VIEW det_*
        HELIQL->>CH: register batch detection SQL
        Note over PROD: Rule is live in stream + batch
        DETENG->>GIT: commit(promotion_record)
    else Below thresholds
        DETENG->>GIT: open tuning PR with observed FP patterns
    end

    Note over DETENG: Continue monitoring; propose retirement if yield drops
```

---

## 7. Data Tiering Flow

How events are routed through hot → warm → cold tiers based on data class and age.

```mermaid
flowchart TD
    EVENT["OCSF Event\n(ingested from Redpanda)"]

    subgraph Classifier["Data Class Routing"]
        DC1["Identity / EDR / Agent telemetry"]
        DC2["Network / Firewall"]
        DC3["Debug / Heartbeat"]
    end

    subgraph Hot["🔥 Hot Tier\n(ClickHouse)"]
        H1["Identity/EDR: 14 days\nSub-second point queries\nSOC console + agent tools"]
        H2["Network: 7 days\nReal-time dashboards"]
    end

    subgraph Warm["🌡️ Warm Tier\n(Iceberg on MinIO/S3)"]
        W1["Identity/EDR: 1 year\nParquet + Iceberg manifests\nClickHouse iceberg() queries"]
        W2["Network: 90 days\nTime-partitioned Parquet"]
        W3["Customer-owned bucket (BYOC)\nCustomer encryption key"]
    end

    subgraph Cold["❄️ Cold Tier\n(S3 Glacier Deep Archive)"]
        C1["Identity/EDR: 7 years\n~$0.001/GB-month\nRe-hydratable in minutes"]
        C2["Network: compliance window"]
    end

    DISCARD["🗑️ Discarded after 24h\n(never persisted)"]

    EVENT --> DC1
    EVENT --> DC2
    EVENT --> DC3

    DC1 --> H1
    DC2 --> H2
    DC3 --> DISCARD

    H1 -->|"after 14 days"| W1
    H2 -->|"after 7 days"| W2

    W1 -->|"after 1 year"| C1
    W2 -->|"after 90 days"| C2

    W1 --> W3
    W2 --> W3

    style Hot fill:#7d1128,stroke:#e94560,color:#fff
    style Warm fill:#1a3a5c,stroke:#0f3460,color:#fff
    style Cold fill:#0d1b2a,stroke:#533483,color:#fff
    style DISCARD fill:#2d2d2d,stroke:#666,color:#aaa
```

---

## 8. Attestation Envelope — Structure

The three attestation envelope variants and their evidence blocks.

```mermaid
classDiagram
    class AttestationEnvelope {
        +String envelope_version
        +Uuid agent_action_id
        +Uuid case_id
        +String tenant_id
        +String agent_id
        +ExecutionPathKind execution_path
        +Verdict verdict
        +EvidenceBlock evidence
        +TimingBlock timing
        +String signature [Ed25519 hex]
        +DateTime signed_at
        +canonical_bytes() Vec~u8~
    }

    class ExecutionPathKind {
        <<enumeration>>
        Classifier
        Llm
        Hybrid
    }

    class Verdict {
        <<enumeration>>
        TruePositive
        FalsePositive
        Benign
        NeedsInvestigation
        EscalatedStub
    }

    class EvidenceBlock {
        <<enumeration>>
        Classifier(ClassifierEvidence)
        Llm(LlmEvidence)
        Hybrid(HybridEvidence)
    }

    class ClassifierEvidence {
        +String model_hash [SHA-256]
        +String feature_hash
        +AlertFeatures input_features
        +Vec~f32~ shap_values
        +f64 raw_score
        +f64 calibrated_score
        +f64 novelty_score
        +bool is_ood
    }

    class AlertFeatures {
        +f64 severity_score
        +f64 source_class_id
        +f64 entity_reputation_score
        +f64 baseline_deviation
        +f64 threat_intel_hit_count
        +f64 hour_of_day
        +f64 asset_criticality
        +f64 prior_disposition_ratio
    }

    class LlmEvidence {
        +String model_provider
        +String model_id
        +Vec~ToolCallRecord~ tool_calls
        +Vec~IntermediateBelief~ intermediate_beliefs
        +Vec~String~ evidence_citations
        +String reasoning_summary
    }

    class ToolCallRecord {
        +String tool_id
        +String args_hash [SHA-256]
        +PolicyDecision policy_decision
        +u32 latency_ms
        +bool result_ok
    }

    class HybridEvidence {
        +ClassifierEvidence classifier_draft
        +LlmEvidence llm_final
        +EscalationReason escalation_reason
    }

    class EscalationReason {
        <<enumeration>>
        LowCalibratedConfidence
        OutOfDistribution
        HighNoveltyScore
        PolicyRequiresLlm
    }

    class TimingBlock {
        +DateTime started_at
        +DateTime ended_at
        +u32 total_ms
        +u32 classifier_ms
        +u32 llm_ms
    }

    AttestationEnvelope --> ExecutionPathKind
    AttestationEnvelope --> Verdict
    AttestationEnvelope --> EvidenceBlock
    AttestationEnvelope --> TimingBlock
    EvidenceBlock --> ClassifierEvidence
    EvidenceBlock --> LlmEvidence
    EvidenceBlock --> HybridEvidence
    ClassifierEvidence --> AlertFeatures
    HybridEvidence --> ClassifierEvidence
    HybridEvidence --> LlmEvidence
    HybridEvidence --> EscalationReason
    LlmEvidence --> ToolCallRecord
```

---

## 9. HELIQL Compilation — Multi-Target Flow

How a single HELIQL rule compiles to three execution targets.

```mermaid
flowchart TD
    RULE["detection: aws_login_anomalous_geo\nwhere: event.auth_status = 'Success'\ncondition: cloud_region NOT IN baseline(user, 90d)\nseverity: medium\nruntime: stream | batch | federated"]

    PARSER["HELIQL Parser\n(pest PEG grammar)"]

    AST["HELIQL AST\nDetection { id, conditions,\nentity_ref, temporal_window,\nmitre_tags, targets }"]

    COMPILER["HELIQL Compiler"]

    subgraph Targets["Compilation Targets"]
        direction LR

        subgraph Stream["⚡ Stream Target"]
            RW_DDL["CREATE MATERIALIZED VIEW\ndet_aws_login_anomalous_geo AS\nSELECT ... FROM cloudtrail_events e\nWHERE auth_status = 'Success'\nAND NOT EXISTS (\n  SELECT 1 FROM entity_baselines eb\n  WHERE eb.actor_user_name = e.actor_user_name\n  AND eb.cloud_region = e.cloud_region\n)"]
            RW_DEPLOY["RisingWave :4566\nDDL deployed on startup"]
        end

        subgraph Batch["📦 Batch Target"]
            CH_SQL["SELECT 'aws_login_anomalous_geo' AS det_id,\n  actor_user_name, cloud_region, event_time\nFROM iceberg('s3://attest-warm/cloudtrail/**')\nWHERE auth_status = 'Success'\n  AND cloud_region NOT IN (\n    SELECT cloud_region FROM ...\n    WHERE ...\n    AND event_time > now() - interval '90 days'\n  )"]
            CH_DEPLOY["ClickHouse\nRun on-demand for hunting\nor scheduled backtest"]
        end

        subgraph Federated["🌐 Federated Target"]
            FED["Push-down to external engine\n(Splunk SPL / KQL / BigQuery SQL)\ngenerated from same AST"]
            FED_DEPLOY["Customer SIEM\nor data lake"]
        end
    end

    SIGMA["Sigma YAML\n(imported via sigma.rs)"]
    SIGMA -->|"parse → HELIQL AST"| AST

    RULE --> PARSER
    PARSER --> AST
    AST --> COMPILER
    COMPILER --> RW_DDL
    COMPILER --> CH_SQL
    COMPILER --> FED

    RW_DDL --> RW_DEPLOY
    CH_SQL --> CH_DEPLOY
    FED --> FED_DEPLOY

    style Targets fill:#0f3460,stroke:#533483,color:#fff
    style Stream fill:#1a3a5c,stroke:#0f3460,color:#fff
    style Batch fill:#1a2a4c,stroke:#0f3460,color:#fff
    style Federated fill:#1a1a3c,stroke:#533483,color:#fff
```

---

## 10. Deployment Topology — Railway MVP

Physical service layout on Railway for a single mid-market tenant.

```mermaid
flowchart TB
    subgraph Customer["Customer Environment"]
        SOURCES["Cloud sources\n(CloudTrail, Okta, EDR)"]
        S3["Customer S3 / GCS\nIceberg warm tier\n(BYOC — customer-owned key)"]
    end

    subgraph Railway["Railway Project (us-east4)"]
        subgraph Ingest["Ingest Layer"]
            COL["attest-collector\nRust :4000\n2-core / 4GB"]
            RP["redpanda\ncp-kafka KRaft\n4-core / 8GB"]
        end

        subgraph Stream["Stream Runtime"]
            RW["risingwave\nRust :4566\n8-core / 32GB\n+ persistent volume"]
            IW["attest-storage-iceberg\nRust (batch writer)\n2-core / 4GB\n(parallel — migration)"]
            ARR["arroyo\nRust SQL streaming :5115\n2-core / 4GB\n(ETL + CEP pipelines)\ndepends on postgres"]
        end

        subgraph Storage["Hot Storage"]
            CH["clickhouse\nNVMe volume\n16-core / 64GB"]
            MINIO["minio\nS3-compatible\n(dev / staging only)\n+ persistent volume"]
        end

        subgraph AgentLayer["Agentic Layer"]
            ORCH["attest-orchestrator\nRust :4300\n4-core / 16GB"]
            MCP_GW["attest-mcp-gateway\nRust :4242\n2-core / 4GB"]
            ML["ml-sidecar\nPython FastAPI :5001\n4-core / 16GB"]
        end

        subgraph API["API + Frontend"]
            CP["attest-control-plane\nRust Axum :8080\n4-core / 8GB"]
            WB["workbench\nNext.js\nNixpacks"]
        end

        subgraph Detection["Detection"]
            DR["attest-detection-runtime\nRust\n2-core / 4GB"]
        end
    end

    subgraph External["External Services"]
        ANT["Anthropic API\nClaude Sonnet (Triager)\nClaude Opus (Investigator)"]
        TI_EXT["Threat Intel APIs\n(VirusTotal, MISP)"]
    end

    SOURCES -->|"HTTPS / Kafka"| COL
    COL -->|"Kafka produce"| RP
    RP -->|"Kafka consume"| RW
    RP -->|"Kafka consume"| IW
    RP -->|"Kafka consume"| ARR
    RP -->|"Kafka consume"| DR
    IW -->|"S3 API"| MINIO
    IW -->|"S3 API (prod)"| S3
    ARR -->|"Parquet (arroyo/ prefix)"| MINIO
    ARR -->|"CEP alerts"| ORCH
    MINIO -->|"Iceberg table function"| CH
    S3 -->|"Iceberg table function"| CH
    RW -->|"entity_baselines / recent_events"| CP
    CH -->|"hot + warm queries"| CP
    DR -->|"Kafka alerts"| ORCH
    ORCH -->|"tool calls"| MCP_GW
    MCP_GW -->|"query_hot_tier"| CH
    MCP_GW -->|"get_user_baseline"| RW
    ORCH -->|"LLM escalation"| ANT
    MCP_GW -->|"threat intel lookup"| TI_EXT
    ORCH -->|"calibrate"| ML
    CP -->|"REST + WebSocket"| WB

    style Customer fill:#1a1a2e,stroke:#e94560,color:#fff
    style Railway fill:#16213e,stroke:#0f3460,color:#fff
    style External fill:#0d1b2a,stroke:#533483,color:#fff
```

---

## 11. Alert State Machine

Lifecycle of an alert from ingestion through resolution.

```mermaid
stateDiagram-v2
    [*] --> Ingested : Event arrives via collector

    Ingested --> Queued : Detection rule fires\n(det_* materialized view)

    Queued --> Triaging : Coordinator assigns to Triager

    Triaging --> AutoClosed : Classifier path\ncalibrated confidence ≥ 0.85\nShadow check: allowed\nVerdict: benign

    Triaging --> ClassifierDispositioned : Classifier path\nconfidence ≥ threshold\nVerdict: true_positive / false_positive

    Triaging --> LlmEscalated : Low confidence OR OOD\nor policy requires LLM

    LlmEscalated --> NeedsInvestigation : LLM verdict:\nneeds_investigation

    LlmEscalated --> ClassifierDispositioned : LLM verdict:\ntrue_positive / false_positive / benign

    NeedsInvestigation --> Investigating : Coordinator assigns Investigator

    Investigating --> PendingHumanReview : Shadow check disagrees\nor high-severity verdict\nrequires human confirmation

    Investigating --> Resolved : Investigator verdict\n+ citations accepted

    ClassifierDispositioned --> PendingHumanReview : Analyst escalates manually

    PendingHumanReview --> Resolved : Human analyst closes

    AutoClosed --> [*] : Attestation envelope signed\nand appended to audit log

    Resolved --> [*] : Attestation envelope signed\nand appended to audit log

    note right of AutoClosed
        Auto-close rate target: ≥ 70% of benign corpus
        False auto-close rate: ≤ 0.5%
    end note

    note right of Triaging
        Classifier path: < 50ms P50
        LLM escalation: < 15s local / < 8s Anthropic
    end note
```

---

## 12. Agent-Aware Detection Fabric (AADF) — New Telemetry Classes

How AI agent telemetry flows through the same Attest platform for AI threat detection.

```mermaid
flowchart LR
    subgraph AIAgents["Customer's AI Agents"]
        A1["LLM Agent A\n(e.g. coding assistant)"]
        A2["LLM Agent B\n(e.g. data analyst)"]
        OTEL_COL["OpenTelemetry\nCollector"]
    end

    subgraph AttestIngest["Attest AADF Ingest"]
        direction TB
        OTEL_EVENTS["GenAI OTEL traces\nPrompt / completion /\ntoken counts / tool calls"]
        MCP_LOGS["MCP server/client logs\nEvery tool call: server,\ntool, args hash, result hash"]
        A2A["A2A protocol traces\nPeer identity, delegated tasks"]
        REASON["Agent reasoning logs\nSystem prompt hash,\nchain-of-thought summaries"]
        RAG["Vector store access logs\nRAG retrievals, doc IDs,\nsimilarity scores"]
    end

    subgraph OCSFNorm["OCSF Normalization"]
        AI_ENT["AIAgent entity\n(new OCSF class)\nmodel, version,\nsystem_prompt_hash,\nallowed_tools, owner"]
    end

    subgraph Detection["AADF Detection Content"]
        LLM01["LLM01: Prompt injection\ndirect + indirect\n(document/email content)"]
        LLM02["LLM02: Sensitive info disclosure\nPII in completions\noutput filter triggers"]
        LLM06["LLM06: Excessive agency\ntool-call anomalies\nprivilege escalation chains"]
        LLM08["LLM08: RAG poisoning\nvector store anomalies"]
        LLM10["LLM10: Unbounded consumption\ncost / loop anomalies"]
    end

    subgraph Correlation["Cross-Domain Correlation"]
        CHAIN["Chained Agent Compromise\nEmail injection (LLM01 low)\n+ anomalous tool call (LLM06 mid)\n+ network egress (classical mid)\n→ HIGH severity case"]
    end

    A1 -->|OTEL traces| OTEL_COL
    A2 -->|MCP logs| OTEL_COL
    OTEL_COL --> OTEL_EVENTS
    OTEL_COL --> MCP_LOGS
    OTEL_COL --> A2A
    OTEL_COL --> REASON
    OTEL_COL --> RAG

    OTEL_EVENTS --> AI_ENT
    MCP_LOGS --> AI_ENT
    A2A --> AI_ENT
    REASON --> AI_ENT
    RAG --> AI_ENT

    AI_ENT --> LLM01
    AI_ENT --> LLM02
    AI_ENT --> LLM06
    AI_ENT --> LLM08
    AI_ENT --> LLM10

    LLM01 --> CHAIN
    LLM06 --> CHAIN
```

---

## Summary — Diagram Index

| # | Diagram | Type | What it shows |
|---|---|---|---|
| 1 | System Context | C4 L1 | Attest in relation to users and external systems |
| 2 | Container Diagram | C4 L2 | All containers across the six planes |
| 3 | Event Journey | Data flow | Raw cloud event → fired detection alert |
| 4 | Agentic Plane | Component | Five agents, policy engine, MCP, attestation wiring |
| 5 | Hybrid Triage | Sequence | Step-by-step classifier path + LLM escalation |
| 6 | SIDM Loop | Sequence | Detection Engineer agent continuous improvement loop |
| 7 | Data Tiering | Flow | Hot → warm → cold tiering policy by data class |
| 8 | Attestation Envelope | Class | Three evidence variants and their structure |
| 9 | HELIQL Compilation | Flow | Single rule → stream + batch + federated targets |
| 10 | Railway Topology | Deployment | Physical service layout, sizing, connections |
| 11 | Alert State Machine | State | Alert lifecycle from ingestion to resolution |
| 12 | AADF | Flow | AI agent telemetry ingestion and detection content |
