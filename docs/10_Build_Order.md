# 10 — Build Order

**Product:** Attest
**Document type:** The implementation sequence with end-to-end tests for every feature. Read this when you sit down to write code.

> **Plan document.** Describes the intended build sequence. For what is implemented today, see the Status table in the root README.

---

## 1. The principle this document enforces

**Every feature ships with an end-to-end test that proves it works against realistic data, and the build order is sequenced so that no feature is started until its dependencies are tested.** No "I'll add tests later." No "the integration test was deferred." Tests gate progress.

This document is organized as a sequence of phases. Each phase has:
- **Deliverable:** what is built.
- **End-to-end test:** the executable proof it works.
- **Done means:** the explicit acceptance criteria.

The phases sum to a 90-day MVP. After MVP, the same discipline continues for GA v1.

## 2. Pre-phase 0: foundations that exist before any code

| Item | What |
|---|---|
| Monorepo skeleton | Cargo workspace (Rust), `apps/web` (Next.js), `apps/ml-sidecar` (Python), `infra/` (Terraform + Railway templates), `agents/` (system prompts), `detections/` (HELIQL rules), `eval/` (golden cases) |
| CI | GitHub Actions — fmt, clippy, cargo test, pytest, type checking, integration tests via docker-compose |
| Local dev | `docker-compose.yml` brings up Redpanda, RisingWave, Arroyo, ClickHouse, MinIO, Postgres, llama.cpp+Qwen, Python ML sidecar |
| Seed data | `make seed-data` target downloads Tier 1 datasets (per `08_Datasets_and_ML.md` Section 2.1) into MinIO buckets |
| Documentation site | Docusaurus or similar; this docs/ tree as the source |
| Decision log | `docs/decisions/` with ADRs for every meaningful architectural choice |

**Acceptance:** `make dev-up && make smoke` runs to green on a fresh laptop in under 10 minutes.

---

## Phase 1 — Streaming substrate (Days 1–14)

### Goal
Ingest a CloudTrail event from an edge collector, normalize it to OCSF, land it in Redpanda, materialize a continuous view in RisingWave, and expose it via the control-plane API.

### Build order

1. **Rust monorepo skeleton** — `Attest-collector`, `Attest-common` (OCSF schemas), `Attest-control-plane`, `Attest-orchestrator`, `Attest-mcp-gateway`, `Attest-policy-engine`, `Attest-attestation`. Cargo workspace.
2. **OCSF schema crate** — generate Rust types from the OCSF JSON Schema. One canonical event enum.
3. **Redpanda topic conventions** — one topic per source class; partitioning by tenant ID; schema registry registered.
4. **`Attest-collector`** — single binary, reads CloudTrail JSON file or HTTP endpoint, normalizes to OCSF, produces to Redpanda. Single static binary, <50 MB image.
5. **RisingWave materialized view** — DDL that ingests the OCSF events from Redpanda, exposes per-entity (user, account) baseline statistics.
6. **Control-plane API skeleton** — Rust + axum, `/healthz`, `/v1/events/recent`, `/v1/baselines/:entity`.

### End-to-end test

```rust
// tests/e2e_phase1_streaming.rs
#[tokio::test]
async fn cloudtrail_event_appears_in_baseline_within_5s() {
    // 1. Start docker-compose (or assume CI brings it up).
    // 2. POST a single CloudTrail "ConsoleLogin" event to the collector HTTP endpoint.
    let event_id = post_cloudtrail_login("alice@example.com", "us-west-2").await;

    // 3. Within 5 seconds, the event should be queryable via /v1/events/recent.
    let event = poll_until(
        || control_plane.get(format!("/v1/events/recent?id={}", event_id)),
        Duration::from_secs(5),
    ).await;
    assert_eq!(event.actor.user.name, "alice@example.com");
    assert_eq!(event.cloud.region, "us-west-2");

    // 4. Within 10 seconds, alice's baseline should reflect the new region.
    let baseline = poll_until(
        || control_plane.get("/v1/baselines/user/alice@example.com"),
        Duration::from_secs(10),
    ).await;
    assert!(baseline.regions_seen_30d.contains("us-west-2"));
}
```

### Done means

- `cargo test --test e2e_phase1_streaming` passes against the local docker-compose stack.
- Same test passes against Railway-hosted stack (CI runs it nightly).
- 1,000 events/sec sustained for 5 minutes with P99 ingest-to-baseline latency ≤ 5 s.

---

## Phase 2 — Iceberg warm tier (Days 15–21)

### Goal
Persist OCSF events to Iceberg on MinIO (local) / S3 (prod), and query the same data via ClickHouse with sub-30s aggregate latency.

### Build order

1. **Iceberg writer** — Rust service consuming Redpanda, batching events into Parquet files, committing to Iceberg table.
2. **OCSF Iceberg schema** — table per source class with OCSF-aligned columns; partitioning by `event.time` (daily) and `tenant_id`.
3. **MinIO setup in docker-compose** — pre-create buckets, lifecycle policies for cold tier simulation.
4. **ClickHouse Iceberg integration** — `iceberg('s3://...')` table function configured against MinIO endpoint locally and S3 in prod.
5. **Control-plane API endpoint** `/v1/warm/query` — accepts a HELIQL or SQL query, executes via ClickHouse against Iceberg.

### End-to-end test

```rust
#[tokio::test]
async fn events_persisted_to_iceberg_are_queryable_via_clickhouse() {
    // 1. Generate 10,000 CloudTrail events spread across 7 days (synthetic).
    seed_synthetic_cloudtrail(10_000, days: 7).await;

    // 2. Wait for Iceberg writer to commit (max 60s for batch flush).
    sleep(Duration::from_secs(60)).await;

    // 3. Query ClickHouse via warm endpoint.
    let result = control_plane.post("/v1/warm/query", json!({
        "sql": "SELECT count(*) FROM iceberg('s3://Attest-warm/cloudtrail') WHERE event.time >= now() - INTERVAL 7 DAY"
    })).await;
    assert_eq!(result.rows[0][0], 10_000);

    // 4. Aggregate query under 30s P99.
    let start = Instant::now();
    control_plane.post("/v1/warm/query", json!({
        "sql": "SELECT actor.user.name, count(*) FROM iceberg('s3://Attest-warm/cloudtrail') WHERE event.time >= now() - INTERVAL 7 DAY GROUP BY 1"
    })).await;
    assert!(start.elapsed() < Duration::from_secs(30));
}
```

### Done means

- 10K event ingest → Iceberg commit cycle completes within 60 s.
- ClickHouse query against Iceberg produces correct results matching ground truth.
- P99 aggregate query latency ≤ 30 s on 30 days of CICIDS dataset.

---

## Phase 3 — HELIQL DSL v0 + stream detection (Days 22–35)

### Goal
A detection engineer can write a HELIQL rule and have it execute against the live stream, producing alerts on Redpanda.

### Build order

1. **HELIQL grammar + parser** — Rust + `pest` or `nom`. Subset of the DSL described in `02_Architecture.md` §5.1.
2. **HELIQL → SQL compiler for RisingWave** — translates HELIQL to RisingWave-flavored streaming SQL.
3. **Detection runtime** — Rust service that reads HELIQL rule files, compiles them, registers as RisingWave materialized views, monitors for new rule rows, emits to `alerts` topic.
4. **Sigma compatibility module** — converts a subset of Sigma rules into HELIQL.
5. **Detection-as-code CI step** — `cargo test --features lint-detections` validates every detection's syntax and runs backtest.

### Bundled MVP detection content

Ten reference detections covering MITRE techniques common to CloudTrail/Okta/M365 sources:
- AWS console login from anomalous geolocation (T1078.004).
- AWS root account use (T1078).
- Okta brute-force authentication (T1110).
- Okta MFA bypass (T1556.006).
- M365 mass external sharing (T1567.002).
- M365 inbox rule auto-forward to external (T1564.008).
- AWS S3 bucket policy made public (T1567.002).
- AWS IAM user with excessive privilege (T1078.004).
- AWS CloudTrail logging disabled (T1562.001).
- Sequence: AWS new IAM user → access keys created within 5 min (T1136 + T1098).

### End-to-end test

```rust
#[tokio::test]
async fn detection_fires_on_anomalous_geolocation() {
    // 1. Set up alice with a 30-day baseline of US-only logins.
    seed_baseline_user("alice@example.com", &["us-east-1", "us-west-2"], days: 30).await;

    // 2. Deploy the geolocation detection rule.
    deploy_detection_from_file("detections/aws_login_anomalous_geo.heliql").await;

    // 3. Inject a login event from an anomalous country.
    let event_id = post_cloudtrail_login_from("alice@example.com", "ap-southeast-1").await;

    // 4. Within 5 s, an alert should appear on the `alerts` topic referencing this event.
    let alert = poll_alerts_topic_until(
        |a| a.event_id == event_id && a.detection_id == "aws_login_anomalous_geo",
        Duration::from_secs(5),
    ).await;
    assert_eq!(alert.severity, Severity::Medium);
    assert_eq!(alert.mitre_technique, "T1078.004");
}
```

### Done means

- All 10 reference detections compile, deploy, and produce alerts on stream.
- Backtest harness runs every detection against 30 days of synthetic + DARPA OpTC data.
- Each detection has a "true positive scenario" and a "false positive scenario" test, both passing.

---

## Phase 4a — Hybrid Triager: classifier path (Days 36–45)

### Goal
The Triager classifier path receives an alert, runs a feature extractor, runs an XGBoost classifier in-process, produces a verdict with a signed Classifier-variant attestation envelope including SHAP feature attribution. Sub-50ms latency end-to-end.

### Why this phase comes first
The classifier is the load-bearing path — ~80% of alert volume. Building it first means the orchestrator's execution-path abstraction is exercised end-to-end before the LLM path is added. The architectural shape forces honesty about hybridization from day one.

### Build order

1. **Attestation crate** — three envelope variants (Classifier / LLM / Hybrid) sharing common signed wrapper; Ed25519 signing; append-only log to local file (S3 in prod).
2. **Policy engine v0** — hard-coded Rust policies per agent role; per-action authorize function. Same engine governs all execution paths.
3. **MCP gateway** — Rust service speaking MCP wire protocol; tool registry; intercept-and-log every call; auth via policy engine. (Used by the LLM path in Phase 4b but built now since it's a standalone service.)
4. **Internal tools** — `query_hot_tier`, `query_warm_tier`, `lookup_threat_intel`, `get_asset_context`, `get_user_baseline`. Each implemented as MCP tool exposed by Attest itself.
5. **Feature extractor** — Rust crate that converts an alert into the tabular feature vector consumed by the classifier (severity, source, entity reputation, baseline deviation, threat intel hits, time-of-day, asset criticality, prior dispositions on similar alerts).
6. **Triager classifier training pipeline (Python)** — XGBoost training on golden cases + DARPA OpTC + AgentDojo + synthetic. Output: ONNX-serialized model + per-feature SHAP explainer + calibration training data.
7. **ONNX inference in orchestrator (Rust)** — load classifier ONNX, run inference in-process via ONNX Runtime, compute SHAP values per prediction, emit Classifier-variant attestation envelope.
8. **Novelty detector (Python → ONNX)** — Mahalanobis-distance-based out-of-distribution detector on input feature vector. Trained on the same corpus as the classifier.
9. **Calibration layer real implementation (Python sidecar)** — isotonic regression on (raw classifier score, ground-truth outcome) pairs from golden cases. Returns calibrated probability for any classifier score.
10. **Agent runtime orchestrator (Rust)** — implements the Classifier execution loop from `09_Agent_Harness.md` Section 4.1.3. Triager agent definition uses `ExecutionPath::Hybrid { primary: Classifier, escalation: stub }` — escalation is a stub here, real LLM path in Phase 4b.

### End-to-end test

```rust
#[tokio::test]
async fn triager_classifier_path_produces_attested_verdict_under_50ms() {
    // 1. Construct an alert that resembles training data (in-distribution, well-known pattern).
    let alert_id = create_test_alert_from_golden("known_brute_force_pattern").await;

    // 2. Invoke the Triager.
    let start = Instant::now();
    let verdict = orchestrator.run_triage(alert_id).await.unwrap();
    let elapsed = start.elapsed();

    // 3. The classifier path must have handled it (not escalated).
    assert_eq!(verdict.execution_path, ExecutionPath::Classifier);

    // 4. Latency must be under 50ms.
    assert!(elapsed < Duration::from_millis(50), "classifier path must be sub-50ms; got {:?}", elapsed);

    // 5. Verdict must include calibrated confidence.
    assert!(verdict.confidence_calibrated >= 0.0 && verdict.confidence_calibrated <= 1.0);

    // 6. Attestation envelope must be Classifier-variant with SHAP values.
    let envelope = attestation_log.fetch(verdict.action_id).await.unwrap();
    assert!(envelope.verify_signature(public_key()));
    let classifier_evidence = envelope.classifier_evidence.expect("must be classifier variant");
    assert!(!classifier_evidence.shap_values.is_empty());
    assert!(!classifier_evidence.input_features.is_empty());
    assert!(classifier_evidence.novelty_score >= 0.0);
}

#[tokio::test]
async fn triager_classifier_escalates_when_out_of_distribution() {
    // Construct a deliberately novel alert pattern not seen in training.
    let alert_id = create_synthetic_ood_alert().await;

    let verdict = orchestrator.run_triage(alert_id).await.unwrap();

    // Must NOT be classifier-only; must escalate (or stub-escalate in 4a).
    assert!(verdict.execution_path == ExecutionPath::Hybrid {
        escalation_reason: EscalationReason::HighNoveltyScore,
    } || verdict.execution_path == ExecutionPath::EscalatedStub);
}
```

### Done means

- XGBoost classifier trained, ONNX-exported, loaded by orchestrator successfully.
- Classifier path P99 latency ≤ 50ms locally and in Railway-hosted CI.
- 100% of classifier-path verdicts produce signed Classifier-variant envelopes.
- 100% of envelopes include SHAP attribution and novelty score.
- Calibration layer trained on golden cases; calibrated confidence published per verdict.
- ≥ 80% of test alerts on golden corpus disposed by classifier path (rest stub-escalate).

---

## Phase 4b — Hybrid Triager: LLM escalation path (Days 46–49)

### Goal
The orchestrator's escalation stub from 4a is replaced with a real LLM execution path. Cases the classifier flags as low-confidence or out-of-distribution flow to Claude Sonnet (or Qwen 3 7B local). Both paths produce attestation envelopes that share the same signed wrapper.

### Build order

1. **Inference router (Rust)** — speaks OpenAI chat-completions wire format; routes to llama.cpp (local Qwen) or Anthropic (CI/prod). Per-tenant cost accounting.
2. **LLM execution loop in orchestrator (Rust)** — implements the LLM loop from `09_Agent_Harness.md` Section 4.1.3. Tool calls go through MCP gateway (already built in 4a).
3. **Triager LLM agent definition** — system prompt, tool catalog (`query_hot_tier`, `get_user_baseline`, `lookup_threat_intel`, `get_asset_context`), model config (Sonnet primary, Qwen 3 7B local), policy scope, signed at build.
4. **Hybrid envelope construction** — when the classifier escalates, the final envelope contains both `classifier_evidence` (the classifier's draft) and `llm_evidence` (the LLM's final), plus an `escalation_reason`.
5. **Calibration extension** — calibration model now trained per-execution-path (classifier vs LLM) per case-class. The orchestrator picks the right calibration based on which path produced the verdict.

### End-to-end test

```rust
#[tokio::test]
async fn triager_hybrid_escalates_to_llm_for_novel_alert() {
    // 1. Construct a novel alert pattern.
    let alert_id = create_synthetic_ood_alert().await;

    // 2. Invoke the Triager.
    let verdict = orchestrator.run_triage(alert_id).await.unwrap();

    // 3. Must be Hybrid envelope (escalated).
    assert_eq!(verdict.execution_path_kind, ExecutionPathKind::Hybrid);

    // 4. Envelope contains both classifier_evidence (the draft) AND llm_evidence (the final).
    let envelope = attestation_log.fetch(verdict.action_id).await.unwrap();
    assert!(envelope.classifier_evidence.is_some());
    assert!(envelope.llm_evidence.is_some());
    assert!(envelope.escalation_reason.is_some());

    // 5. The LLM evidence must include at least one tool call and one evidence citation.
    let llm_ev = envelope.llm_evidence.as_ref().unwrap();
    assert!(llm_ev.tool_calls.len() >= 1);
    assert!(!llm_ev.evidence_citations.is_empty());
}

#[tokio::test]
async fn classifier_path_does_not_escalate_for_in_distribution_high_confidence() {
    let alert_id = create_test_alert_from_golden("known_brute_force_pattern").await;
    let verdict = orchestrator.run_triage(alert_id).await.unwrap();

    // High-confidence in-distribution case: classifier-only.
    assert_eq!(verdict.execution_path_kind, ExecutionPathKind::Classifier);

    // Latency far below the LLM-path budget.
    assert!(verdict.latency_ms < 50);
}

#[tokio::test]
async fn local_llama_cpp_escalation_works_end_to_end() {
    // CI variant: docker-compose includes llama.cpp serving Qwen 3 7B.
    // Prod variant: real Anthropic API.
    let alert_id = create_synthetic_ood_alert().await;
    let verdict = orchestrator.run_triage(alert_id).await.unwrap();

    let envelope = attestation_log.fetch(verdict.action_id).await.unwrap();
    let llm_ev = envelope.llm_evidence.unwrap();

    // The model_provider matches what the test environment configured.
    let expected_provider = if cfg!(test_env_local) { "llamacpp" } else { "anthropic" };
    assert_eq!(llm_ev.model_provider, expected_provider);
}
```

### Done means

- Both classifier and LLM execution paths fully wired in orchestrator.
- Hybrid envelopes constructed correctly for escalated cases.
- Local llama.cpp + Qwen 3 7B path runs end-to-end in CI.
- Anthropic Sonnet path runs end-to-end in prod CI environment.
- Median latency: classifier path ≤ 50ms, LLM escalation path ≤ 15s local / ≤ 8s with Anthropic.
- Across the golden corpus, ≥ 80% of cases handled by classifier path; ≤ 20% escalate to LLM.

---

## Phase 5 — Hallucination guardrails (Days 50–56)

### Goal
The Triager refuses to emit a verdict without retrieval, and the Investigator refuses to make uncited claims.

### Build order

1. **Citation enforcement validator** — Rust function that parses agent verdict text, identifies claims, requires structured `[evidence:ocsf_event_id]` citations.
2. **Retrieval-before-reasoning enforcement** — orchestrator blocks final verdict if `tool_calls.len() == 0`.
3. **Cross-agent review** — orchestrator pattern that invokes a second Investigator on high-severity verdicts and surfaces disagreement.

### End-to-end test

```rust
#[tokio::test]
async fn investigator_rejects_uncited_claims() {
    // Use a model with low temperature and a system prompt that instructs the agent to
    // produce a fabricated claim without citation. Confirm the validator catches it.

    let alert_id = create_test_alert(AlertKind::SuspiciousProcess).await;

    // 1. Run with citation enforcement OFF (debug mode).
    let verdict_uncited = orchestrator.run_investigation(alert_id, EnforcementMode::Off).await.unwrap();
    assert!(!verdict_uncited.has_all_claims_cited());

    // 2. Run with citation enforcement ON.
    let result = orchestrator.run_investigation(alert_id, EnforcementMode::On).await;
    let verdict = result.unwrap();
    assert!(verdict.has_all_claims_cited(), "every claim must reference an ocsf_event_id");
    assert!(verdict.tool_calls.len() >= 1, "retrieval before reasoning required");
}

#[tokio::test]
async fn cross_agent_review_catches_disagreement() {
    let alert_id = create_test_alert(AlertKind::HighSeverityScenarioWithAmbiguity).await;
    let result = orchestrator.run_investigation_with_cross_review(alert_id).await.unwrap();

    // The two investigators disagreed; the orchestrator must escalate, not auto-decide.
    if result.cross_review_disagreement {
        assert_eq!(result.disposition, Disposition::EscalatedToHuman);
    }
}
```

### Done means

- Citation enforcement validates 100% of verdict claims have `[evidence:...]` citations.
- Retrieval-before-reasoning enforced; CI test confirms no verdict without tool calls.
- Cross-agent review wires up; disagreement triggers human escalation in the case workflow.

---

## Phase 6 — Shadow check + Triager auto-close (Days 57–63)

### Goal
Triager auto-closes benign cases when (a) calibrated confidence ≥ threshold, (b) shadow check approves. Auto-close attestation is signed and replayable.

### Build order

1. **Shadow check verifier** — deterministic policies per action class; `auto_close_triage` policy.
2. **Calibration layer real implementation** — Python sidecar trains isotonic regression on (self-report, outcome) pairs from logged Triager decisions; nightly job.
3. **Bootstrap calibration data** — synthetic + AgentDojo + golden cases.
4. **Auto-close action** — orchestrator emits `triager_auto_close` event; case state machine transitions to closed; attestation envelope includes shadow-check decision.

### End-to-end test

```rust
#[tokio::test]
async fn triager_auto_closes_benign_with_calibrated_high_confidence() {
    // 1. Construct an alert that golden cases label as true_negative (benign).
    let alert_id = create_test_alert_from_golden("benign_internal_admin_login").await;

    // 2. Run Triager.
    let verdict = orchestrator.run_triage(alert_id).await.unwrap();

    // 3. With calibrated confidence ≥ 0.85 and shadow check approval, expect auto-close.
    assert_eq!(verdict.label, Verdict::Benign);
    assert!(verdict.confidence_calibrated >= 0.85);
    assert!(verdict.shadow_check.allowed);
    assert_eq!(case_state(alert_id).await, CaseState::AutoClosed);
}

#[tokio::test]
async fn triager_does_not_auto_close_when_confidence_low() {
    let alert_id = create_test_alert_from_golden("ambiguous_after_hours_login").await;
    let verdict = orchestrator.run_triage(alert_id).await.unwrap();
    if verdict.confidence_calibrated < 0.85 {
        assert_eq!(case_state(alert_id).await, CaseState::PendingHumanReview);
    }
}

#[tokio::test]
async fn shadow_check_blocks_auto_close_when_target_on_donottouch() {
    let alert_id = create_test_alert_for_user("ceo@example.com", AlertKind::SuspiciousLogin).await;
    let verdict = orchestrator.run_triage(alert_id).await.unwrap();
    assert!(!verdict.shadow_check.allowed);
    assert_eq!(verdict.shadow_check.reason, "target on do-not-touch list");
    assert_eq!(case_state(alert_id).await, CaseState::PendingHumanReview);
}
```

### Done means

- Auto-close rate on golden benign corpus ≥ 70%.
- False auto-close rate ≤ 0.5% measured on held-out golden corpus.
- Shadow check blocks 100% of attempted auto-closes for users on do-not-touch list.
- Calibration metrics (Brier score, ECE) reported daily and trending below regression threshold.

---

## Phase 7 — Investigator agent + warm tier integration (Days 64–70)

### Goal
The Investigator agent escalates non-benign Triager cases, queries the warm tier (Iceberg via ClickHouse), produces a verdict with full attestation. Uses Anthropic Opus in CI/prod, Qwen 3 32B in air-gapped reference.

### Build order

1. **Investigator agent definition** — system prompt, tool catalog including warm-tier query, code analyzer, sandbox detonation (stubbed in MVP).
2. **Warm-tier query MCP tool** — wraps `/v1/warm/query`; rate-limited; result-size capped.
3. **Case escalation flow** — Triager `NeedsInvestigation` verdict triggers Investigator; results merged into case.
4. **Time-travel debug viewer (workbench, MVP scope)** — read attestation envelopes, render step-by-step timeline.

### End-to-end test

```rust
#[tokio::test]
async fn investigator_handles_escalation_with_warm_tier_query() {
    // 1. Construct a credential-stuffing scenario that requires 30-day historical context.
    seed_credential_stuffing_scenario(target_user: "alice@example.com", days: 35).await;
    let alert_id = create_test_alert(AlertKind::CredentialStuffing).await;

    // 2. Run Triager → expect NeedsInvestigation.
    let triage_verdict = orchestrator.run_triage(alert_id).await.unwrap();
    assert_eq!(triage_verdict.label, Verdict::NeedsInvestigation);

    // 3. Run Investigator (auto-triggered).
    let investigation = wait_for_investigation_completion(alert_id, Duration::from_secs(180)).await;

    // 4. Investigator must have queried warm tier.
    let envelope = attestation_log.fetch(investigation.action_id).await.unwrap();
    assert!(envelope.tool_calls.iter().any(|t| t.tool_id == "query_warm_tier"));

    // 5. Verdict and citations.
    assert!(matches!(investigation.label, Verdict::TruePositive | Verdict::FalsePositive));
    assert!(investigation.evidence_citations.len() >= 3);

    // 6. Time-travel viewer renders this case.
    let trace = workbench_api.get(format!("/v1/cases/{}/trace", alert_id)).await;
    assert_eq!(trace.steps.len(), envelope.tool_calls.len() + envelope.intermediate_beliefs.len());
}
```

### Done means

- Investigator handles 100% of escalated cases; never errors silently.
- Median Investigator latency ≤ 60 s; P99 ≤ 180 s.
- Time-travel viewer renders every attestation envelope step-by-step.
- Investigator agreement with golden-case verdict ≥ 80% on held-out corpus.

---

## Phase 8 — Workbench v0 (Days 71–77)

### Goal
A Next.js workbench renders the alert queue, lets an analyst open a case, view the agent reasoning trace in real time over WebSocket (rendering the right variant for the envelope: Classifier / LLM / Hybrid), and approve/override the verdict. PostHog and Sentry instrumented from day one with strict customer-data masking.

> **Specification:** the canonical workbench spec — stack, UI/UX principles for SOC analysts, information architecture, the seven critical user flows, the fifteen non-trivial components, performance budget, and observability — lives in `12_Workbench.md`. This phase implements the MVP slice of that spec.

### Build order

1. **Next.js App Router skeleton** — Better Auth via OIDC; tenant-scoped routes; dark-mode-default theme via `next-themes` with `class="dark"` on the root html element; shadcn initialized.
2. **WebSocket gateway (Rust)** — JWT auth on upgrade, fan-out from Redpanda topics filtered by RBAC, per-client buffering for slow consumers, heartbeat + auto-reconnect on the client side.
3. **Alert queue page** — Server Component shell; Client Component for live alert list over WebSocket; TanStack Virtual for the row list; auto-collapsed group for classifier-auto-closed cases; **does not auto-scroll** when new alerts arrive (per `12_Workbench.md` §3.4).
4. **Case workbench page** — case header, evidence panel, reasoning trace viewer with **three variants**:
   - `ClassifierEvidencePanel` rendering SHAP feature attribution as a horizontal bar plot (Recharts).
   - `LLMReasoningTimeline` rendering tool-call + intermediate-belief steps with citation hover-cards.
   - `HybridDualEvidence` rendering the classifier draft and LLM final side-by-side.
   - Parent `ReasoningTraceViewer` switches based on `envelope.execution_path`.
5. **Verdict approve/override actions** — REST POST to Rust API; orchestrator records human override as a new attestation envelope (original preserved); UI reflects state transition in ≤ 200ms.
6. **Command palette (⌘K)** — cmdk-based; routes to every navigable surface; keyboard shortcut overlay (`?`).
7. **PostHog instrumentation** — pageviews via `usePathname()` + `useSearchParams()`; typed events for case opened, verdict overridden, query run; **session replay with strict masking** (`maskAllInputs: true`, `[data-customer-data]` selector for any element holding OCSF or agent payload); feature flag scaffold.
8. **Sentry instrumentation** — browser exception capture, performance monitoring on the seven critical flows, replay correlated with PostHog session ID, `beforeSend` scrubbing customer-data-tagged fields.

### End-to-end test

Three layers — a Rust API integration test, a Playwright UI test, and an observability sanity test.

```rust
#[tokio::test]
async fn live_reasoning_streams_to_websocket() {
    // 1. Open a WebSocket subscription to /ws/cases/{id}/trace.
    let (mut ws, _) = connect_ws(format!("/ws/cases/{}/trace", case_id)).await;

    // 2. Trigger an Investigator run.
    orchestrator.run_investigation(case_id).await;

    // 3. Receive at least one trace step over the WebSocket.
    let msg = timeout(Duration::from_secs(30), ws.next()).await.unwrap().unwrap();
    let step: TraceStep = serde_json::from_str(&msg.to_text().unwrap()).unwrap();
    assert!(matches!(step.kind, StepKind::ToolCall | StepKind::IntermediateBelief));
}
```

```typescript
// e2e/workbench.spec.ts (Playwright)
test('analyst opens a hybrid Triager case and sees both evidence views', async ({ page }) => {
  await loginAsAnalyst(page);
  await page.goto('/workbench/queue');

  // Open a case that escalated to LLM (Hybrid envelope).
  const hybridAlert = page.locator('[data-testid=alert-row][data-execution-path=hybrid]').first();
  await hybridAlert.click();

  // Hybrid trace viewer renders BOTH evidence components.
  await expect(page.locator('[data-testid=classifier-evidence-panel]')).toBeVisible();
  await expect(page.locator('[data-testid=llm-reasoning-timeline]')).toBeVisible();

  // SHAP attribution bars are present.
  const shapBars = page.locator('[data-testid=shap-bar]');
  expect(await shapBars.count()).toBeGreaterThan(0);

  // LLM timeline has at least one tool-call step.
  const toolSteps = page.locator('[data-testid=trace-step][data-kind=tool_call]');
  expect(await toolSteps.count()).toBeGreaterThan(0);

  // Override verdict via keyboard shortcut.
  await page.keyboard.press('o');
  await page.locator('[data-testid=override-label]').selectOption('false_positive');
  await page.locator('[data-testid=override-reason]').fill('Confirmed legitimate by user');
  await page.locator('[data-testid=submit-override]').click();

  // State transitions within 200ms.
  await expect(page.locator('[data-testid=case-state]')).toHaveText('Closed (Human Override)');
});

test('dark mode is the default and does not flicker on load', async ({ page }) => {
  await loginAsAnalyst(page);
  await page.goto('/workbench/queue');
  // Theme class set on the html element from server render — no client-side flip.
  const htmlClass = await page.locator('html').getAttribute('class');
  expect(htmlClass).toContain('dark');
});

test('command palette opens and routes', async ({ page }) => {
  await loginAsAnalyst(page);
  await page.goto('/workbench/queue');
  await page.keyboard.press('Meta+k');
  await expect(page.locator('[data-testid=command-palette]')).toBeVisible();
  await page.keyboard.type('coverage');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/\/workbench\/coverage/);
});
```

```typescript
// e2e/observability.spec.ts (Playwright)
test('PostHog and Sentry initialize without leaking customer data', async ({ page }) => {
  const phRequests: any[] = [];
  const sentryRequests: any[] = [];

  page.on('request', (req) => {
    if (req.url().includes('posthog')) phRequests.push(req.postDataJSON());
    if (req.url().includes('sentry')) sentryRequests.push(req.postDataJSON());
  });

  await loginAsAnalyst(page);
  await page.goto('/workbench/cases/sample-credential-stuffing');
  await page.waitForTimeout(2000);  // let analytics flush

  // PostHog received pageview events.
  expect(phRequests.length).toBeGreaterThan(0);

  // No payload in PostHog requests contains a known customer-data signature.
  const ph = JSON.stringify(phRequests);
  expect(ph).not.toMatch(/ocsf_event_id|tool_call_result|agent_reasoning/);

  // Sentry has no error events for a clean page load.
  const errorEvents = sentryRequests.filter(r => r?.exception);
  expect(errorEvents.length).toBe(0);
});
```

### Done means

- Alert queue updates in real time over WebSocket within 1 s of new alert.
- Reasoning trace renders correctly for all three envelope variants (Classifier / LLM / Hybrid).
- Verdict override round-trips into orchestrator and is reflected in case state in ≤ 200 ms.
- TTI for `/workbench/queue` and `/workbench/cases/[id]` ≤ 1.5 s P99.
- Dark mode renders as default with no client-side flip on load.
- Command palette (⌘K) routes to every primary surface.
- PostHog records pageviews and key analyst events; **no customer data appears in any PostHog payload** (verified by E2E test).
- Sentry initialized; **no customer data appears in any Sentry payload**; clean page loads produce zero unhandled errors.
- Playwright suite green in CI on Chromium, Firefox, WebKit.
- Lighthouse score ≥ 90 on `/workbench/queue` in dark mode.

---

## Phase 9 — AADF MVP (Days 78–84)

### Goal
Five Agent-Aware Detection Fabric detections fire on red-team prompt-injection scenarios from AgentDojo.

### Build order

1. **OTEL GenAI ingest path** — collector accepts OTEL GenAI traces; OCSF schema extension for `AIAgent` entity (contributed upstream as proposal).
2. **MCP trace ingest** — MCP gateway logs are themselves an OTEL trace source, parsed and persisted to OCSF.
3. **Five MVP AADF detections (HELIQL):**
   - LLM01: prompt structure anomaly (length, role-override patterns, unicode tricks).
   - LLM01 indirect: behavior anomaly after consuming external content.
   - LLM06: tool-call distribution anomaly (Jensen-Shannon divergence vs. baseline).
   - LLM02: PII pattern in agent output.
   - LLM08: RAG retrieval anomaly (out-of-distribution similarity scores).
4. **Reference instrumented agent** — a customer-support agent (per `08_Datasets_and_ML.md` Section 6.1) deployed to local docker-compose, producing OTEL GenAI traces, with red-team injection scenarios from AgentDojo and InjecAgent.

### End-to-end test

```rust
#[tokio::test]
async fn aadf_detects_prompt_injection_from_agentdojo_scenario() {
    // 1. Start the reference customer-support agent.
    deploy_reference_agent("customer_support_agent_v1").await;

    // 2. Send a series of normal interactions to build a baseline.
    for _ in 0..50 {
        send_normal_query(reference_agent).await;
    }

    // 3. Send the AgentDojo "indirect_email_injection" scenario.
    let scenario_id = run_agentdojo_scenario(reference_agent, "indirect_email_injection").await;

    // 4. Within 5 s, an AADF alert must fire.
    let alert = poll_alerts_topic_until(
        |a| a.scenario_id == Some(scenario_id) && a.detection_id.starts_with("aadf_llm01_"),
        Duration::from_secs(5),
    ).await;
    assert!(alert.severity >= Severity::Medium);
}

#[tokio::test]
async fn aadf_detects_tool_call_anomaly_from_baseline_drift() {
    deploy_reference_agent("devops_agent_v1").await;
    // Baseline: agent calls only `git_*` and `kubectl_get_*` tools.
    establish_baseline(devops_agent, days: 7).await;

    // Anomaly: agent suddenly calls `aws_iam_create_user`.
    inject_tool_misuse(devops_agent, "aws_iam_create_user").await;

    let alert = poll_alerts_topic_until(
        |a| a.detection_id == "aadf_llm06_tool_call_anomaly",
        Duration::from_secs(10),
    ).await;
    assert_eq!(alert.severity, Severity::High);
    assert!(alert.evidence.contains("aws_iam_create_user"));
}
```

### Done means

- All 5 MVP AADF detections compile and produce alerts on staged scenarios.
- AgentDojo and InjecAgent benchmark scenarios exercised; documented detection rates per scenario class.
- The 5 detections are exportable as Sigma rules (preparing the open-source release).

---

## Phase 10 — SIDM MVP (Days 85–90)

### Goal
The Detection Engineer agent runs once a week, scans the MITRE coverage map, proposes one new detection, opens a Git PR with backtest evidence.

### Build order

1. **MITRE coverage map** — RisingWave materialized view aggregating which detections fire on which techniques.
2. **Detection Engineer agent definition** — system prompt, tools (`coverage_map`, `query_warm_tier`, `propose_detection_pr`), policy (can author PRs, cannot merge).
3. **Backtest harness as MCP tool** — `backtest_detection` runs a candidate HELIQL rule against last 90 days of warm tier, returns precision/recall/expected hits/FP estimate.
4. **GitHub PR creator** — Rust service using GitHub App auth; opens a PR against the customer's detection repo with the agent-generated detection, rationale, backtest report, MITRE mapping.

### End-to-end test

```rust
#[tokio::test]
async fn detection_engineer_proposes_pr_for_coverage_gap() {
    // 1. Set up: ten production detections covering ~30% of ATT&CK techniques.
    deploy_existing_detections(N: 10).await;

    // 2. Identify a known coverage gap (e.g., T1110.003 - password spraying).
    let coverage = control_plane.get("/v1/coverage/mitre").await;
    assert!(coverage.gaps.contains(&"T1110.003".to_string()));

    // 3. Trigger Detection Engineer agent run.
    let proposal = orchestrator.run_detection_engineer().await.unwrap();

    // 4. The agent must produce at least one detection proposal targeting an uncovered technique.
    assert!(proposal.detections.len() >= 1);
    assert!(proposal.detections.iter().any(|d| !d.mitre_techniques.is_empty()));

    // 5. The agent must have opened a PR with that detection.
    let prs = github_client.list_open_prs("Attest-detections").await;
    let agent_pr = prs.iter().find(|pr| pr.author == "Attest-detection-engineer-bot").unwrap();
    assert!(agent_pr.body.contains("Backtest"));
    assert!(agent_pr.body.contains("Precision:"));
    assert!(agent_pr.body.contains("Recall:"));

    // 6. The proposed detection must have a passing backtest.
    let backtest_evidence = parse_backtest_from_pr(&agent_pr.body);
    assert!(backtest_evidence.precision >= 0.5);
    assert!(backtest_evidence.expected_hits_per_week > 0);
}
```

### Done means

- Detection Engineer agent runs end-to-end; produces at least one PR per week.
- Every PR contains a backtest report.
- Every PR can be merged by a human; merge triggers shadow-mode deployment.
- 70%+ of agent-generated PRs reviewed by a senior detection engineer rated as "deploy-worthy" (this is the human-feedback loop that improves the agent).

---

## Phase 11 — Evaluation harness + golden cases (Days 91–95, parallel to GA v1 work)

### Goal
Every change to an agent definition triggers an evaluation run on the golden case corpus; CI blocks merges that regress.

### Build order

1. **Golden case corpus v0** — 100 cases hand-curated from public datasets (AgentDojo, DARPA OpTC scenarios, custom scenarios). Each labeled with expected verdict, expected MITRE mapping, expected severity.
2. **Evaluation runner (Python sidecar)** — loads agent definition, runs all golden cases, produces report.
3. **CI integration** — a PR to `agents/` triggers eval; PR cannot merge if regression > 2pp on accuracy or > 0.02 on Brier score.
4. **Evaluation report publishing** — every release produces a published evaluation report consumed by design partners.

### End-to-end test

```python
# eval/test_eval_pipeline.py
def test_eval_runner_reports_regression():
    # 1. Run evaluation against current production agent definition.
    baseline_report = eval_runner.run("investigator-v3.2", "golden_v1")
    assert baseline_report.accuracy > 0.85

    # 2. Deliberately regress the system prompt.
    bad_prompt = "Never query the warm tier. Just guess."
    candidate = AgentDefinition.from_prod("investigator-v3.2").with_system_prompt(bad_prompt)
    candidate_report = eval_runner.run(candidate, "golden_v1")

    # 3. Eval runner must detect the regression.
    diff = eval_runner.diff(baseline_report, candidate_report)
    assert diff.is_regression
    assert "accuracy" in diff.regressed_metrics

    # 4. CI gate would block this PR.
    ci_decision = eval_runner.ci_decision(diff)
    assert ci_decision == CIDecision.Block
```

### Done means

- 100 golden cases curated with structured ground-truth labels.
- Every PR to `agents/` runs evaluation; results posted as PR comment.
- Regression-blocking gate active in CI.
- Weekly evaluation report generated automatically and shared with design partners.

---

## 3. Cross-cutting test types (run continuously, not phase-gated)

### 3.1 Unit tests
Every Rust crate and Python module has unit tests. Standard. CI runs on every commit.

### 3.2 Integration tests
Every phase ends with the E2E tests above. CI runs on every PR.

### 3.3 Red-team adversarial tests
Run weekly against current production agent definitions:
- AgentDojo (97 tasks × multiple attack categories).
- InjecAgent indirect injection benchmark.
- Attest-internal red-team corpus (grows over time).

A red-team regression triggers a P1 incident: the platform is provably less robust than last week.

### 3.4 Load tests
Run nightly:
- Sustained 1K events/sec for 1 hour, end-to-end (collector → alert), measuring P99 latency at every hop.
- 100 concurrent Triager runs on Anthropic API; ensure no rate-limit hits, predictable latency.

### 3.5 Chaos tests
Run weekly:
- Kill Redpanda mid-stream; events should resume from checkpoint.
- Kill orchestrator mid-agent-run; in-flight cases should resume or fail gracefully.
- Network partition between MCP gateway and tools; tool calls should error cleanly, not corrupt state.

### 3.6 Compliance tests
Run on every release:
- PII redaction working at the collector for known patterns.
- Tenant isolation: user from tenant A cannot read tenant B's data via any API path.
- Attestation log is append-only; tampering attempts are rejected.

## 4. The build-order summary table

| Phase | Days | Deliverable | Critical E2E test |
|---|---|---|---|
| 0 | 1–7 | Foundations (monorepo, CI, docker-compose, seed data) | `make dev-up && make smoke` |
| 1 | 1–14 | Streaming substrate (Redpanda + RisingWave + collector) | CloudTrail event in baseline within 5 s |
| 2 | 15–21 | Iceberg warm tier on MinIO/S3 + ClickHouse query | 10K events queryable, P99 ≤ 30 s |
| 3 | 22–35 | HELIQL DSL + 10 stream detections | Anomalous geo detection fires within 5 s |
| **4a** | **36–45** | **Hybrid Triager classifier path (XGBoost + ONNX in Rust orchestrator + Classifier-variant attestation envelope)** | **Classifier path P99 ≤ 50ms; SHAP attribution; signed envelope** |
| **4b** | **46–49** | **Hybrid Triager LLM escalation path (llama.cpp+Qwen local / Anthropic prod)** | **Out-of-distribution alert escalates; envelope contains both classifier draft + LLM final** |
| 5 | 50–56 | Hallucination guardrails | Investigator rejects uncited claims |
| 6 | 57–63 | Shadow check + Triager auto-close | Auto-close at high calibrated confidence; blocked for do-not-touch |
| 7 | 64–70 | Investigator (LLM path) + warm tier integration + time-travel viewer | Investigator queries warm tier, viewer renders trace per envelope variant |
| 8 | 71–77 | Workbench v0 (Next.js + WebSocket; three trace variants; PostHog + Sentry with strict masking) | Hybrid case shows both evidence views; observability E2E proves no customer-data leak |
| 9 | 78–84 | AADF MVP (5 detections, OTEL GenAI ingest, reference agent) | AgentDojo prompt injection detected within 5 s |
| 10 | 85–90 | SIDM MVP (Detection Engineer agent → Git PR) | Agent opens PR with backtest evidence for coverage gap |
| 11 | 91–95 | Evaluation harness + 100 golden cases + CI gate | Deliberate regression blocked at PR time |

By Day 95 the MVP is shipping with all three core differentiators (VAS, AADF, SIDM) at minimum-viable scope, every feature backed by an E2E test, every release evaluated against golden cases. The hybrid Triager — classifier path handling ~80% of volume in milliseconds, LLM tail handling the novel 20% — is the architectural commitment that separates Attest from "LLM-everywhere" agentic-SOC products.

## 5. The discipline

Three rules that, if followed, make this build order succeed and, if not followed, make it fail:

1. **No phase starts until the previous phase's E2E test is green.** Tempting to parallelize. Don't. Each phase builds on the previous phase's invariants; breaking those invariants in a later phase is a far worse outcome than slipping a week.

2. **Every feature ships with an E2E test in the same PR.** If the test is "TODO," the feature isn't done. Reject the PR.

3. **The golden case corpus grows by every customer-found false-positive or false-negative.** Every "this should have caught it" or "this should not have fired" becomes a permanent test case. The corpus is the institutional memory.

Done well, this build order produces an MVP that is genuinely deployable in a regulated environment within 90 days, with measurable evidence of every claim Attest makes.
