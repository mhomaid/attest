# 09 — Agent Harness

**Product:** Attest
**Document type:** Specification of the agent runtime, evaluation, and verification framework. The technical spine of the Verifiable Agentic SOC.

> **Design document.** Describes the target architecture. For what is implemented today, see the Status table in the root README.

---

## 1. Why this document exists

The original architecture document (`02_Architecture.md`) and AI strategy document (`03_AI_Agentic_Strategy.md`) describe attestation, calibration, shadow checks, and evaluation in pieces, scattered across sections. Engineers building the agentic plane need a single technical specification.

**This is that specification.** It is the contract between the product promise (Verifiable Agentic SOC) and the implementation.

## 2. What an "agent harness" is, in Attest terms

A harness is the runtime that surrounds every agent execution to make agents:

- **Verifiable.** Every action produces a signed, replayable attestation.
- **Calibrated.** Every confidence number is mapped to a real probability of correctness.
- **Bounded.** Every action passes a deterministic shadow check before execution.
- **Evaluated.** Every prompt, model, or tool change is benchmarked against golden cases before deployment.
- **Observed.** Attest's own agents are themselves visible in the AADF.

**Without the harness, "agentic SIEM" reduces to "LLM with tools and a UI."** The harness is what makes Attest's verifiability claim defensible in front of a regulated buyer.

## 2.5 LLMs only where they earn their place

A foundational principle that shapes the rest of this document: **Attest uses LLMs only where they are demonstrably the best tool for the job, and explicitly avoids them everywhere else.** This is not a cost optimization. It is a correctness, latency, and verifiability commitment.

**Where LLMs are the right tool (and we use them):**
- Deep multi-step investigation reasoning (Investigator agent)
- Open-ended hypothesis generation (Hunter agent)
- Detection authoring from natural-language intent (Detection Engineer agent)
- Natural-language query → HELIQL compilation (workbench)
- Investigation summary / case narrative generation (across agents)
- Triage of *novel or ambiguous* alerts (the LLM tail of a hybrid Triager)

**Where LLMs are the wrong tool (and we don't use them):**
- Streaming detection rule evaluation — RisingWave SQL, sub-100ms
- Behavioral baselining (UEBA) — robust statistics, sub-millisecond
- Tool-call anomaly detection — Jensen-Shannon divergence on distributions
- Network anomaly classification — XGBoost on flow features
- Calibration — isotonic regression, ~1ms
- Shadow check verification — deterministic Rust, ~1ms (an LLM here would defeat the safety perimeter)
- MCP gateway, policy engine, attestation, audit logging — pure plumbing
- **Triage of well-known patterns (the bulk of alert volume)** — a calibrated XGBoost classifier on alert features outperforms an LLM on every dimension that matters: latency (10ms vs 8s), cost, determinism, native feature attribution

The Triager is the canonical example of where the marketing of agentic SOC misleads the market. The right Triager is **a hybrid**: a fast classifier on the well-known 80%, an LLM on the novel/ambiguous 20%. Section 4.1 specifies this in detail.

This principle has three consequences worth stating up front:

1. **Latency budget.** ~95% of alert volume in Attest is processed end-to-end without ever invoking an LLM. Median time-to-disposition for those cases is under 2 seconds. LLM latency only enters the budget for the cases where LLM judgment is actually needed.
2. **Verifiability story.** Customers and regulators get *two* verifiable paths, not one. Classifier decisions come with SHAP feature attributions; LLM decisions come with reasoning traces and citations. Both produce the same attestation envelope schema.
3. **Honest competitive positioning.** Most agentic-SOC competitors pitch "LLMs everywhere." Attest pitches "LLMs where they earn it." This is a stronger position with regulated buyers and CISOs who have grown tired of LLM theater.

## 3. Components of the harness

```
              ┌────────────────────────────────────────────────────────────┐
              │                Agent Harness (Rust + Python)               │
              │                                                            │
   request →  │  ┌──────────────────┐                                      │
              │  │ Agent Runtime    │ ─── invokes ─────┐                   │
              │  │ (orchestrator)   │                  ▼                   │
              │  └────────┬─────────┘         ┌────────────────┐           │
              │           │                   │  LLM provider  │           │
              │           ▼                   │  (Anthropic /  │           │
              │  ┌──────────────────┐         │   vLLM / llama │           │
              │  │ Policy Engine    │         │   .cpp)        │           │
              │  │ (deterministic)  │         └────────────────┘           │
              │  └────────┬─────────┘                                      │
              │           │ allowed?                                       │
              │           ▼                                                │
              │  ┌──────────────────┐         ┌────────────────┐           │
              │  │ MCP Gateway      │ ─────── │ Tool registry  │           │
              │  └────────┬─────────┘         └────────────────┘           │
              │           │ tool call                                      │
              │           ▼                                                │
              │  ┌──────────────────┐                                      │
              │  │ Attestation      │ ──────► append-only log              │
              │  │ Builder          │                                      │
              │  └────────┬─────────┘                                      │
              │           ▼                                                │
              │  ┌──────────────────┐         ┌────────────────┐           │
              │  │ Calibration      │ ◄────── │ ML sidecar     │           │
              │  │ Layer            │         │ (Python)       │           │
              │  └────────┬─────────┘         └────────────────┘           │
              │           ▼                                                │
              │  ┌──────────────────┐                                      │
              │  │ Shadow Check     │  if action requested                 │
              │  │ Verifier         │                                      │
              │  └────────┬─────────┘                                      │
              │           ▼                                                │
              │     verdict + signed attestation                           │
              └────────────────────────────────────────────────────────────┘
```

**Language choices:**
- The orchestrator, policy engine, MCP gateway, attestation builder, and shadow check verifier are all **Rust** services — they sit on the hot path and need predictable latency.
- The calibration layer and red-team / evaluation runner are **Python** — they live in the ML sidecar and don't need millisecond latency.

## 4. Component specifications

### 4.1 Agent Runtime (orchestrator)

**Language:** Rust (axum + tokio)
**Responsibility:** Receive a case-handling request, route it to the appropriate execution path for the agent role, mediate tool calls through the MCP gateway, emit attestation.

#### 4.1.1 Execution paths

A single agent role can have multiple **execution paths** with different cost, latency, and capability profiles. The orchestrator routes each request to the right path. Both paths produce the same attestation envelope schema, so downstream verification is uniform.

| Path kind | Use cases | Latency | Cost | Determinism | Native attribution |
|---|---|---|---|---|---|
| `Classifier` | Well-known patterns; high-volume routine triage | <50ms | ~$0 | High | SHAP per feature |
| `Llm` | Novel cases; deep investigation; authoring; NL→code | 1s–3min | $0.001–$0.20/case | Low | Cited reasoning trace |
| `Hybrid` | Triage with classifier-first, LLM-on-low-confidence escalation | <50ms for ~80% of cases; LLM tail for the rest | Mixed | Per-case | Per-path |

**The Triager is `Hybrid` by default. The Investigator, Hunter, and Detection Engineer are `Llm`. The Coordinator is `Classifier` (it's lightweight routing logic, not reasoning).**

#### 4.1.2 Agent definition (revised)

```rust
pub struct AgentDefinition {
    pub id: AgentId,                    // e.g., "triager-hybrid-v3.2"
    pub role: AgentRole,                // Triager | Investigator | Hunter | DetectionEngineer | Responder | Coordinator
    pub execution: ExecutionPath,       // Classifier | Llm | Hybrid
    pub policy_scope: PolicyScopeRef,
}

pub enum ExecutionPath {
    Classifier {
        model_artifact: ClassifierArtifact,   // ONNX-serialized XGBoost or similar
        model_version_hash: Sha256,
        feature_extractor_hash: Sha256,
        confidence_threshold_for_disposition: f32,
    },
    Llm {
        provider: ModelProvider,              // Anthropic | OpenAI | Bedrock | VLLM | LlamaCpp
        model_id: String,
        model_version_hash: Sha256,
        system_prompt_hash: Sha256,
        tool_catalog: Vec<ToolDescriptor>,
        max_iterations: u8,
        max_tokens: u32,
        temperature: f32,
    },
    Hybrid {
        primary: Box<ExecutionPath>,           // typically Classifier
        escalation: Box<ExecutionPath>,        // typically Llm
        escalation_threshold: f32,             // calibrated confidence below which we escalate
        novelty_detector: NoveltyDetectorRef,  // out-of-distribution check on input features
    },
}
```

Agent definitions are **versioned in Git, signed at build time, immutable at runtime.** A change to a system prompt, classifier weights, or threshold produces a new agent definition with a new ID. Old IDs remain valid for replay.

#### 4.1.3 The execution loops

**Classifier path** (used by Triager primary, Coordinator):

```
features = feature_extractor.extract(case)
prediction = classifier.predict(features)               // ~10ms
shap_values = classifier.explain(features, prediction)  // ~20ms
calibrated = calibration_layer.calibrate(agent.id, case_class, prediction.confidence)
if action_requested(prediction):
    shadow_decision = shadow_check.verify(prediction.action)
    if shadow_decision.disagrees:
        escalate_to_human(case, shadow_decision)
        return
emit_attestation_classifier(case, agent, features, shap_values, prediction, calibrated)
```

**LLM path** (used by Investigator, Hunter, Detection Engineer, Triager escalation):

```
while iterations < max_iterations:
    response = llm.chat(messages, tools=tool_catalog)
    record_intermediate_belief(response.content, response.confidence_self_report)

    if response.tool_calls:
        for tool_call in response.tool_calls:
            policy_decision = policy_engine.authorize(agent.id, tool_call)
            if policy_decision.denied:
                record_denial(tool_call, policy_decision.reason)
                messages.append(tool_call_denial_message)
                continue
            result = mcp_gateway.invoke(tool_call)
            record_tool_call(tool_call, result)
            messages.append(result_message)
    elif response.is_final_verdict:
        calibrated = calibration_layer.calibrate(agent.id, case_class, response.confidence_self_report)
        if action_requested(response):
            shadow_decision = shadow_check.verify(response.action)
            if shadow_decision.disagrees:
                escalate_to_human(case, shadow_decision)
                break
        emit_attestation_llm(case, agent, messages, response, calibrated)
        break
    iterations += 1
```

**Hybrid path** (used by Triager):

```
// Primary: classifier
features = feature_extractor.extract(case)
prediction = classifier.predict(features)
calibrated = calibration_layer.calibrate(agent.id, case_class, prediction.confidence)
novelty = novelty_detector.score(features)            // out-of-distribution check

if calibrated >= escalation_threshold AND novelty < novelty_threshold:
    // Confident classifier disposition — done.
    emit_attestation_classifier(...)
    return

// Otherwise: escalate to LLM path
record_escalation_reason(calibrated, novelty)
run_llm_path(case, agent.escalation)
// Attestation envelope captures both: the classifier's prediction AND the LLM's verdict
emit_attestation_hybrid(...)
```

#### 4.1.4 Why this is in Rust

Every alert hits the orchestrator's routing logic. The classifier path runs in-process (ONNX Runtime in Rust) without leaving the orchestrator process. The LLM path makes outbound calls to the inference router but the orchestrator itself remains pure Rust. Predictable latency (no GC pauses), low memory per concurrent loop, one orchestrator process can handle thousands of concurrent agent executions on a Railway-sized box.

#### 4.1.5 Attestation envelope variants

Three envelope variants, all sharing the same outer wrapper for verification:

```json
// Common wrapper
{
  "envelope_version": "1.1",
  "agent_action_id": "...",
  "case_id": "...",
  "tenant_id": "...",
  "execution_path": "classifier" | "llm" | "hybrid",
  "agent": { ... },
  "timing": { ... },
  "final_verdict": { ... },
  "shadow_check": { ... },
  "signature": "ed25519:..."
}
```

Classifier-path-specific fields:
```json
"classifier_evidence": {
  "model_artifact_hash": "sha256:...",
  "feature_extractor_hash": "sha256:...",
  "input_features": { "feature_a": 0.42, ... },
  "shap_values": { "feature_a": 0.12, "feature_b": -0.08, ... },
  "raw_prediction": 0.87,
  "novelty_score": 0.12
}
```

LLM-path-specific fields (unchanged from original spec):
```json
"llm_evidence": {
  "model_provider": "anthropic",
  "model_id": "claude-opus-4.7",
  "model_version_hash": "sha256:...",
  "system_prompt_hash": "sha256:...",
  "tool_calls": [ ... ],
  "intermediate_beliefs": [ ... ],
  "evidence_citations": [ ... ]
}
```

Hybrid envelopes contain both `classifier_evidence` and `llm_evidence`, plus an `escalation_reason`.

The time-travel debugger renders all three uniformly — for classifier paths it shows the SHAP feature attributions; for LLM paths it shows the reasoning trace; for hybrid it shows the classifier's "draft" verdict and the LLM's "final" verdict side by side. **This dual-evidence presentation is one of Attest's strongest auditor-facing features.**

### 4.2 Policy Engine (deterministic)

**Language:** Rust
**Responsibility:** Authorize every tool call against per-agent scope, per-tenant policy, per-action risk thresholds. **Never invokes an LLM.** This is the firewall between agent autonomy and customer infrastructure.

**Policy expressed as code (Rego-like or pure Rust):**

```rust
// Example: Responder agent policy for IdP session revocation
fn authorize_responder_action(action: &ResponderAction, ctx: &PolicyContext) -> PolicyDecision {
    if action.kind != ActionKind::IdpRevokeSession {
        return PolicyDecision::deny("not in scope for Responder");
    }
    if ctx.calibrated_confidence < 0.85 {
        return PolicyDecision::deny("calibrated confidence below 0.85 threshold");
    }
    if ctx.target_user.is_in_do_not_touch_list() {
        return PolicyDecision::deny("target on do-not-touch list");
    }
    if ctx.recent_actions_count_last_hour() > 10 {
        return PolicyDecision::deny("rate limit: max 10 actions/hour");
    }
    if ctx.affected_population_size() > 50 {
        return PolicyDecision::escalate("blast radius >50; human approval required");
    }
    PolicyDecision::allow()
}
```

**Key properties:**
- All inputs to a policy decision are explicit and logged.
- Policy versions are tagged in the attestation envelope.
- Policy code is reviewed by humans through Git PR; agents cannot modify their own policy.
- Policy denials produce structured reasons, not just "denied."

**MVP scope:** hard-coded policies in Rust per agent role. **GA v1:** customer-editable policies via a typed DSL (Rego or similar) with version control.

### 4.3 MCP Gateway

**Language:** Rust (thin layer)
**Responsibility:** Single entry point for every tool call; authenticates against the policy engine; logs every invocation; standardizes tool schemas; rate limits.

**Three classes of tools:**

| Class | Description | Examples |
|---|---|---|
| **Internal Attest tools** | Built into Attest | `query_hot_tier`, `query_warm_tier`, `lookup_threat_intel`, `get_asset_context`, `propose_detection_pr` |
| **Customer-allowed external MCP servers** | Customer-approved third-party MCP servers | A SOAR vendor's MCP server, an internal asset-management MCP server |
| **Restricted action tools** | Destructive action tools | `idp_revoke_session`, `edr_isolate_host`, `firewall_block_ioc` |

**Every tool call is logged with:**
- agent ID, action ID, tool ID
- arguments (or hash, if sensitive)
- result (or hash)
- latency
- policy decision and version
- timestamp

The MCP gateway is also a CRITICAL **observability point**: this is where Attest's own AADF observes Attest's own agents. The same logging schema customer agents produce for the AADF, Attest produces for itself.

### 4.4 Attestation Builder

**Language:** Rust
**Responsibility:** Construct, sign, and persist the attestation envelope for every agent action.

**Envelope schema:** the attestation envelope has three variants depending on the execution path that produced the verdict — Classifier, LLM, or Hybrid. The schemas are specified in Section 4.1.5. All three share a common signed wrapper so verification logic is uniform regardless of path.

For an LLM-path Investigator action, the envelope contains `llm_evidence` with the model identifier, system prompt hash, tool calls, intermediate beliefs, and evidence citations. For a Classifier-path Triager action, the envelope contains `classifier_evidence` with the model artifact hash, input features, SHAP values, raw prediction, and novelty score. For a Hybrid Triager action that escalated to LLM, the envelope contains both blocks plus an `escalation_reason`.

**Storage:** append-only log with periodic Sigstore-style transparency-log anchoring (Merkle root published per epoch). Customer can prove no envelope was modified or deleted.

**Replay:** the time-travel debugger (UI feature in workbench) reads the envelope, fetches all referenced data by hash, and reconstructs the agent's view of the world at decision time. For classifier paths the replay shows the SHAP attribution; for LLM paths the reasoning trace; for hybrid the side-by-side comparison of classifier draft and LLM final.

### 4.5 Calibration Layer

**Language:** Rust caller; Python implementation in ML sidecar
**Responsibility:** Map raw confidence scores (from either path) to calibrated probability of correctness.

**Algorithm:** Per-agent, per-execution-path, per-case-class isotonic regression. Trained nightly on (raw confidence, ground-truth outcome) pairs.

**Why one calibration layer for both paths:** the Triager XGBoost classifier produces a probability score that is not natively well-calibrated either (XGBoost tends toward over-confident scores at the extremes). LLMs are notoriously poor at expressing their own uncertainty. **Both need post-hoc calibration against actual outcomes**, and isotonic regression is the right tool for both. Separate calibration models per path (classifier vs LLM) capture their distinct miscalibration patterns.

**Why per-case-class:** an Investigator's confidence on credential-stuffing cases calibrates very differently from its confidence on insider-threat cases. Pooling them hides important miscalibration. The same applies to the Triager classifier across alert categories.

**Cold-start problem:** new agents have no calibration data. We seed with:
- A reasonable prior (assume some level of overconfidence; apply a fixed shrinkage toward 0.5).
- Cross-customer transfer learning: aggregate calibration from anonymized cross-customer data when contractually permitted.
- For classifier paths specifically, calibration data is generated during training via held-out validation set, so the cold-start period is much shorter than for LLMs.

**Metrics published to customer (per agent, per execution path, per case-class):**
- Brier score.
- Expected Calibration Error (ECE).
- Reliability diagram (visual).
- Sample size warning when calibration is based on <100 observations.

This is one of Attest's strongest differentiators for regulated buyers: **a customer can audit the calibration of every decision-making path the platform exposes**. No competitor publishes this.

### 4.6 Shadow Check Verifier

**Language:** Rust
**Responsibility:** For any agent-initiated action above a configurable risk threshold, run a parallel deterministic check before execution.

**Deterministic checks per action class:**

| Action class | Shadow check |
|---|---|
| `idp_revoke_session` | Target not on do-not-touch list; rate-limit OK; calibrated confidence ≥ threshold |
| `edr_isolate_host` | Host not on production-critical list; not on CEO/exec list; blast radius bounded |
| `firewall_block_ioc` | IOC not on internal-asset list; not in known-good third-party list |
| `detection_deploy` | Backtest precision ≥ threshold; no rule conflict; shadow-mode metrics meet promotion criteria |
| `mass_remediation` | Population size below limit; rate limit; manual approval flag honored |

**Shadow-check disagreement protocol:**

1. Agent proposes action.
2. Shadow check runs in parallel.
3. If shadow check says "allow" → action executes; attestation records concordance.
4. If shadow check says "deny" → action blocks; case escalates to human; attestation records discordance.
5. If shadow check says "escalate" (e.g., high blast radius) → action paused for human approval.

**A note on philosophy:** the shadow check is **not** trying to be smarter than the agent. It is enforcing a deterministic safety perimeter. The agent's reasoning is rich; the shadow check's reasoning is auditable. Together they provide both intelligence and accountability.

### 4.7 Evaluation Runner

**Language:** Python (in ML sidecar)
**Responsibility:** Continuously run agent definitions against evaluation corpora; produce regression reports; gate releases.

**Evaluation modes:**

- **Pre-release:** every change to an agent definition (system prompt, model, tool catalog, policy) triggers a CI evaluation run against the golden case corpus. PR cannot merge if regression exceeds tolerance.
- **Continuous (production):** sampled production cases are mirrored to a shadow runner using candidate agent definitions; behavioral diffs surface for review.
- **Red-team (scheduled):** AgentDojo, InjecAgent, and Attest-internal red-team corpora run weekly against current production agents.
- **Customer-specific:** each customer's small (~50 case) corpus runs on every release candidate; release does not roll to that customer if their corpus regresses.

**Evaluation report format (per release per agent):**

```
Agent: investigator-v3.3
Compared to: investigator-v3.2

Golden cases (n=120):
  Verdict accuracy:        92.5%  (was 91.7%, +0.8 pp)
  Citation rate:           99.2%  (was 99.5%, -0.3 pp)
  Avg tool calls per case: 4.3    (was 4.1, +0.2)
  Avg latency:             47s    (was 49s, -2s)

Red-team (AgentDojo, n=97):
  Robustness score:        88.0%  (was 86.5%, +1.5 pp)

Calibration (per case-class):
  credential_stuffing:  Brier 0.18  (was 0.19)
  insider_threat:       Brier 0.24  (was 0.21)  ⚠ regression
  ...

Decision: HOLD  (insider_threat calibration regressed; investigate)
```

This report is human-readable and machine-parseable. It is also published to design partners during the early release cycle as evidence of Attest's evaluation discipline.

### 4.8 Hallucination Guardrails

These are not a separate component; they are runtime invariants enforced at multiple layers.

**Citation enforcement:**
- The Investigator's system prompt mandates: "Every claim referencing data must cite an `ocsf_event_id`."
- Before emitting a final verdict, a Rust validator parses the verdict and rejects any claim without a structured citation.
- Rejection forces the agent to retry with a stronger citation prompt. After N rejections, escalate to human.

**Retrieval before reasoning:**
- The orchestrator enforces that any agent operating on case data must call at least one retrieval tool (hot/warm tier query) before producing a final verdict. The agent cannot reason from prior alone.
- Implementation: tool-call counter; `if final_verdict and tool_calls == 0: reject and re-prompt`.

**Cross-agent review for high-impact verdicts:**
- For verdicts above a configurable severity threshold, the orchestrator independently invokes a second Investigator instance with the same case data and prompts it to critique the first verdict.
- Disagreement → escalate to human, regardless of either agent's confidence.

## 5. The minimum viable harness for MVP

The MVP version ships with reduced scope but full architectural shape. Specifically:

| Component | MVP scope |
|---|---|
| Agent Runtime (Rust) | ✅ Full: orchestrator loop with three execution paths (Classifier / LLM / Hybrid), agent definition versioning |
| **Triager classifier (XGBoost)** | ✅ **Trained on golden cases + DARPA OpTC + AgentDojo; ONNX-serialized; runs in-process via ONNX Runtime in the orchestrator; SHAP attribution computed per prediction** |
| **Triager LLM escalation path** | ✅ **Claude Sonnet (or Qwen 3 7B local) for cases the classifier flags as low-confidence or out-of-distribution** |
| Investigator (LLM path) | ✅ Claude Opus (or Qwen 3 32B air-gapped) |
| Policy Engine (Rust) | ✅ Hard-coded policies per agent role; customer-editable in GA v1 |
| MCP Gateway (Rust) | ✅ Full: every tool call mediated and logged |
| Attestation Builder (Rust) | ✅ All three envelope variants (Classifier/LLM/Hybrid); Ed25519 signing; append-only log; transparency-log anchoring deferred to GA v1 |
| Calibration Layer | ✅ Isotonic regression for both classifier and LLM paths; nightly retrain; cross-customer cold-start deferred to GA v1 |
| Shadow Check Verifier (Rust) | ✅ For Triager auto-close decisions (any path) in MVP; full Responder ships in GA v1 |
| Evaluation Runner (Python) | ✅ Pre-release on golden cases + red-team; customer-specific evals deferred to GA v1 |
| Hallucination Guardrails | ✅ Citation enforcement and retrieval-before-reasoning for LLM paths in MVP |
| Time-travel Debugger (UI) | ✅ Reads any of the three envelope variants; renders SHAP attribution for classifier paths and reasoning trace for LLM paths; alternative-branch exploration deferred to GA v1 |

**MVP team allocation for the harness:** 1 Rust engineer for ~3 weeks builds the orchestrator (with execution-path abstraction) + policy engine + MCP gateway + attestation builder. 1 Python ML engineer for ~3 weeks trains the Triager classifier + builds the calibration layer + evaluation runner.
**This is small.** The reason it's small is that the architectural decisions are made up front; the implementation follows mechanically. The Triager classifier is the single most consequential ML deliverable in the MVP — it determines whether ~80% of alert volume processes in milliseconds or seconds, and whether Attest's unit economics work at scale.

## 6. Local development experience

Every developer can run the entire harness on their laptop:

```
$ git clone Attest && cd Attest
$ make seed-data         # downloads Tier 1 datasets, ~10 GB, cached
$ docker compose up -d   # brings up Redpanda, RisingWave, ClickHouse, MinIO, Postgres, llama.cpp
$ make seed-agents       # registers agent definitions; starts orchestrator
$ cargo run --bin Attest-cli -- run-case --case sample-credential-stuffing
```

The CLI invocation runs the full Triager → Investigator → Responder loop locally with **Qwen 3 7B served from llama.cpp**, produces a real attestation envelope on disk, runs the shadow check, produces a real reasoning trace viewable in the workbench. **No mocks.**

Iteration cycle for an agent prompt change:
1. Edit `agents/investigator.system_prompt.md`.
2. Run `make eval-investigator` — runs the golden case corpus against the new prompt locally on Qwen, emits regression report.
3. If green, push; CI runs the same eval against Anthropic API.
4. If green, merge.

This is the velocity engine. Without it, the team will be hand-debugging agents in production. With it, agent quality improves measurably each week.

## 7. The boundary of what the harness does and doesn't do

**Does:**
- Provides reproducibility of every agent decision.
- Calibrates confidence against ground truth.
- Verifies high-impact actions against deterministic policies.
- Evaluates agent definitions against fixed corpora.
- Catches certain hallucinations (uncited claims, reasoning without retrieval).

**Does not (these are still hard problems):**
- Make weak LLMs strong. A bad model still produces bad reasoning; the harness just catches it more often.
- Eliminate all hallucinations. Catches some structural failures; semantic correctness of cited evidence still depends on the model.
- Replace human review for novel attack patterns. The harness raises confidence, not certainty.
- Train better models. Training is in `08_Datasets_and_ML.md`.

The harness is necessary but not sufficient for trustworthy agents. The other half is the data, models, and prompts. **Together, they constitute the Verifiable Agentic SOC.**

## 8. The single sentence

> **The agent harness is the runtime that turns "an LLM with tools" into "a verifiable, calibrated, bounded, evaluated, observed defender that a regulated CISO can deploy in production."**

Build it once, build it well, and every agent Attest ships afterward inherits the trustworthiness for free.
