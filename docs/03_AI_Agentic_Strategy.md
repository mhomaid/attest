# 03 — AI & Agentic Strategy

**Product:** Attest
**Document type:** AI strategy and agent design philosophy. Read this if you want to understand *why* the agentic plane is built the way it is.

---

## 1. The thesis

Most "AI-native SIEM" and "agentic SOC" products on the market today are honest first attempts. They take an LLM, give it some tools, point it at alerts, and wrap a UI around the results. This works in demos and falls over at enterprise scale for three reasons:

- **Trust deficit.** Regulated CISOs cannot accept "the agent decided." They need provenance, reproducibility, and a way to defend the decision in an audit.
- **Coverage blindspot.** Current AI-SOC products defend against attacks on classical infrastructure but are themselves blind to attacks on AI infrastructure. They treat their own agents as trusted while the rest of the industry is racing to attack them.
- **Static loop.** Detection engineering is still mostly manual. The "AI" is in triage, not in the perpetual evolution of detection content.

Attest is built on three explicit AI commitments that address these failures, in order:

- **AI is governed, not trusted.** Every agent action is verifiable, calibrated, and shadow-checked.
- **AI is observed, not assumed innocent.** AI agents are themselves a detected entity class.
- **AI is the engine of detection improvement, not just triage.** A continuous loop where agents author, test, deploy, and tune detections.

The rest of this document explains how.

## 2. Principles

These principles are non-negotiable. They are the difference between a real verifiable agentic SIEM and AI security theater.

### P1 — Agents act through governed tools, never raw API access

No agent has direct cloud credentials, direct database access, or direct response capability. All agent activity is mediated by:

- An **MCP gateway** that authenticates tools, normalizes their schemas, and rate-limits.
- A **policy engine** (non-LLM, deterministic) that authorizes each tool call against per-agent scope, per-tenant policy, and per-action risk thresholds.
- An **attestation layer** that signs and stores every tool call.

This is the technical equivalent of "the agent has a passport, not master keys."

### P2 — Every agent decision is reproducible from a signed trace

Reproducibility is the foundation of auditability. Every output an agent produces — verdict, recommendation, action — is accompanied by an attestation that captures *enough* state to deterministically replay the decision under the original conditions:

- Model identifier and version hash.
- System prompt hash.
- Inputs (or hashes thereof) for every tool call.
- Outputs (or hashes thereof) for every tool call.
- The agent's chain of intermediate beliefs at each step.
- The final verdict and calibrated confidence.
- The policy version under which the action was authorized.

Stored in an append-only, tamper-evident log. The customer can query: *"show me the exact reasoning that led to last Tuesday's containment of Server 4711."* In ninety seconds.

### P3 — Confidence must be calibrated, not self-reported

LLMs are notoriously poor at expressing their own uncertainty. A model that says "0.95 confidence" may be right 60% of the time on certain case classes. We do not let raw self-reported confidence drive autonomy thresholds. Every agent's confidence is run through a calibration model (isotonic regression, per agent, per case-class) that maps self-reported confidence to **calibrated probability of correctness against ground-truth outcomes**. Brier score and Expected Calibration Error (ECE) are tracked per agent and exposed to the customer.

### P4 — High-impact actions get a deterministic shadow check

Agents are smart, but smart is not the same as safe. Any agent action above a configurable risk threshold goes through a parallel deterministic check before execution:

- Is the target on a "do-not-touch" list? (CEO laptop, PCI workload, life-safety system)
- Does the action exceed blast-radius limits? (no more than N hosts, no more than N% of population)
- Does the action match a deny-by-default policy?
- Has rate-limiting been respected?

If the deterministic check disagrees with the agent, the action pauses for human review. This is the safety net that lets a CISO sleep.

### P5 — Hallucinations are caught, not hoped against

- **Citation enforcement:** every claim must reference a specific OCSF event ID. Uncited claims are rejected by the agent's own validation step.
- **Retrieval before reasoning:** agents are constrained to retrieve evidence first, then reason on it. They are not allowed to reason from training-data prior alone for case verdicts.
- **Cross-agent review:** for high-impact verdicts, a second specialist independently reviews the first's trace before the verdict is finalized.
- **Disagreement signal:** when the second agent disagrees, the case escalates to a human, regardless of confidence.

### P6 — Specialization beats one big agent

A single mega-agent that does everything is the wrong design for three reasons: cost (all volume goes through the most expensive tier), governance (one policy scope is too coarse), and quality (different tasks reward different prompts, models, and tool catalogs). Attest uses five specialists with distinct prompts, models, tool catalogs, and policies, coordinated by a planner.

### P7 — Humans set policy. Agents execute within policy. Agents cannot modify policy

This is the architectural firewall. The policy engine is external to the LLM stack and is updated only through Git PRs reviewed by humans. An agent can request a policy change (and the request is logged), but cannot enact one.

### P8 — Agents are observed and audited like any other identity

Attest's own agents are themselves subject to the Agent-Aware Detection Fabric. If an agent goes rogue — through prompt injection of input data, supply-chain compromise of a tool, or a bug — the same detection content that finds rogue customer agents will find ours. We eat our own dog food, and every customer audit can verify it.

## 3. Agent taxonomy

Five specialists, one coordinator. Each is a distinct deployment artifact: system prompt, model selection, tool catalog, policy scope, evaluation criteria.

### 3.1 The Coordinator (planner)

- **Role:** receive an alert or analyst request, decide which specialists to invoke, in what order, with what context.
- **Model:** mid-tier reasoning model (cost-balanced).
- **Output:** a plan, not a verdict. The plan is itself attested.
- **Tools:** read-only access to case state and a "specialist invocation" tool.
- **Policy:** can invoke other agents but cannot directly act on customer infrastructure.

### 3.2 The Triager (hybrid)

- **Role:** first-pass disposition for every alert. Decide: benign / true positive / needs investigation / needs hunting.
- **Execution:** Hybrid — XGBoost classifier primary path (~80% of volume, sub-50ms) + LLM escalation path (Claude Sonnet / Qwen 3 7B local) for cases the classifier flags as low-confidence or out-of-distribution. See `09_Agent_Harness.md` Sections 2.5 and 4.1, and `08_Datasets_and_ML.md` Section 3.1.
- **Tools (LLM path only):** hot-tier query, threat intel lookup, asset context, identity context.
- **Policy:** can auto-close benign cases above a calibrated-confidence threshold (regardless of which path produced the verdict); everything else escalates.
- **KPI:** classifier-path P99 latency ≤ 50ms; LLM-escalation P99 latency ≤ 30s; autonomous closure rate ≥ 80%; false-positive autonomous closure ≤ 0.1%.

### 3.3 The Investigator

- **Role:** deep investigation for escalated cases.
- **Model:** strong reasoning (Opus-class), with extended thinking for complex chains.
- **Tools:** Triager tools + warm-tier queries, packet/payload decoder, code analyzer, sandbox detonation.
- **Policy:** can request response actions but never executes them directly; recommendations go to the Responder.
- **KPI:** verdict quality measured by analyst agreement rate ≥ 90% on sampled review.

### 3.4 The Hunter

- **Role:** proactive, hypothesis-driven hunting.
- **Model:** strong reasoning + extended thinking.
- **Tools:** federated query, threat intel feeds, MITRE knowledge graph, custom rule executor, historical batch query.
- **Policy:** read-only across all data; cannot deploy detections (those go through the Detection Engineer).
- **KPI:** novel true-positive findings per week per tenant; analyst-rated value of hypotheses surfaced.

### 3.5 The Detection Engineer

- **Role:** continuous detection content lifecycle (see Self-Improving Detection Mesh, Section 7).
- **Model:** strong reasoning + code-capable.
- **Tools:** DSL compiler, backtest harness, coverage map, Git PR creator, ATT&CK knowledge graph.
- **Policy:** can author detections and open PRs; cannot merge without human approval.
- **KPI:** net-new detections per week; precision/recall of merged detections; coverage uplift vs. ATT&CK.

### 3.6 The Responder

- **Role:** execute containment and remediation actions.
- **Model:** fast/cheap; reasoning is constrained to action mapping, not open-ended.
- **Tools:** SOAR connector, IdP connector (revoke session, disable account), EDR connector (isolate host), network connector (block IOC).
- **Policy:** **most restrictive of all agents.** Every action requires:
  - Calibrated confidence above threshold.
  - Deterministic shadow check passed.
  - Within blast-radius limits.
  - Not on do-not-touch list.
  - Optional human approval for actions above tier-2 risk.
- **KPI:** zero unauthorized destructive actions; mean time to containment.

## 4. The verifiable decision lifecycle

Every alert flows through this lifecycle. Each transition is attested.

```
[Alert generated]
     │
     ▼
[Coordinator → plans specialist invocation]
     │  (attested)
     ▼
[Triager investigates]
     │  (attested with citations + calibrated confidence)
     ▼
 ┌───────────────────────────────┐
 │ confidence ≥ threshold        │
 │ AND verdict == benign         │  ──→ [Auto-close, attested]
 └───────────────────────────────┘
     │ otherwise
     ▼
[Investigator deep-dives]
     │  (attested; cross-agent review for high-impact)
     ▼
[Hunter expands scope if pattern detected]
     │  (attested; may produce new cases)
     ▼
[Detection Engineer auto-files PR if coverage gap detected]
     │
     ▼
[Responder proposes containment]
     │  ──→ [Shadow check]  ──→ [Policy check]  ──→ [Optional human approval]
     │
     ▼
[Action executed; full attestation chain stored]
     │
     ▼
[Outcome captured for calibration loop]
```

## 5. Detection of AI threats — the AADF in practice

The Agent-Aware Detection Fabric is the new detective capability. It builds on the same streaming substrate as classical detection but with new telemetry, new entities, and new content.

### 5.1 New telemetry

- **OpenTelemetry GenAI** — standard semantics for prompts, completions, tool calls.
- **MCP traces** — every tool invocation through MCP, with arguments and results hashed.
- **A2A traces** — every agent-to-agent message, with peer identities.
- **Agent reasoning logs** — system prompts, intermediate beliefs (configurable verbosity).
- **Vector store access logs** — RAG retrievals, document IDs.

### 5.2 New entities

`AIAgent` is a first-class OCSF entity (we contribute the schema upstream). Distinct from the underlying service account. Carries `model`, `model_version`, `system_prompt_hash`, `allowed_tools`, `owner`.

### 5.3 New baselines

Per-agent behavioral profiles built continuously:

- Tool-call distribution (which, how often, parameter shape).
- Target distribution (which APIs, which records, which destinations).
- Output size distribution.
- Latency distribution.
- Inter-agent communication graph and frequency.

### 5.4 Canonical detections (ship at GA)

Mapped to OWASP LLM Top 10 with Sigma-compatible export:

- **LLM01 Prompt injection (direct).** Anomalous prompt patterns: long instructions, nested directives, role-override patterns, out-of-scope queries.
- **LLM01 Prompt injection (indirect).** Anomalies in agent behavior shortly after consuming external content (email, web page, document).
- **LLM02 Sensitive information disclosure.** Output filtering for PII patterns, secrets, embeddings of internal knowledge.
- **LLM06 Excessive agency — tool misuse.** Tool-call anomalies vs. baseline (volume, sequence, target).
- **LLM06 Excessive agency — privilege escalation chains.** Agent invokes a sequence of tools that, while individually permitted, in aggregate exceed the agent's intended scope.
- **LLM07 System prompt leakage.** Detection of system prompt content appearing in agent outputs.
- **LLM08 Vector / embedding weaknesses.** Anomalous RAG retrievals, document poisoning indicators.
- **LLM09 Misinformation / hallucination on high-stakes outputs.** Cross-checking specific factual claims (account numbers, policy text) against authoritative sources.
- **LLM10 Unbounded consumption.** Cost or loop anomalies suggesting denial-of-wallet.
- **A2A session smuggling.** Detection of mid-conversation directive injection between peer agents.

These detections are **open-sourced** as Sigma rules. Seeding the category benefits Attest more than holding the content back.

### 5.5 Cross-domain correlation

The decisive feature: joining agent telemetry with classical telemetry into single chained cases. Examples:

- LLM01 (indirect injection in email content) + LLM06 (anomalous tool call to billing API) + classical (network egress to unknown destination by the underlying service account) → single case "agent compromise via email, exfiltration via billing API."
- LLM07 (system prompt leak) + classical (login from new geolocation by agent owner) → single case "credential compromise of agent owner with potential theft of agent IP."

No existing SIEM produces these joins because none has the agent telemetry schema natively.

## 6. Self-Improving Detection Mesh — the SIDM in practice

The Detection Engineer agent runs continuously. Its loop:

1. **Coverage scan.** Reads the MITRE ATT&CK matrix, marks which techniques have how many active detections, flags weak spots.
2. **Threat intel scan.** Ingests TTPs from threat intel feeds, CISA advisories, CVE chatter, internal incident reports.
3. **Outcome scan.** Reads last 7 days of investigations: which true positives had no detection fire? Which detections fired with low precision (TP rate <50%)?
4. **Hypothesis generation.** Produces candidate detections in HELIQL with rationale tied to ATT&CK technique IDs and threat intel.
5. **Backtest.** Runs each candidate against last 90 days of Iceberg data. Reports precision, recall, expected hits/week, FP/week.
6. **PR creation.** Opens a Git PR per detection with full rationale, backtest report, MITRE mapping, expected operational cost.
7. **Human gate.** Detection engineer reviews, approves, merges. Or rejects with feedback that becomes a fine-tuning signal for the agent.
8. **Shadow deploy.** On merge, the detection runs in shadow mode for 7 days. Generates alerts to a "shadow" stream that does not page humans, but feeds the calibration loop.
9. **Promotion.** Auto-promotes to production if precision and recall meet thresholds; otherwise opens a refinement PR.
10. **Continuous tuning.** Monitors signal drift, proposes threshold updates, proposes retirement of low-yield detections.

The detection engineering team **does not disappear.** Their job evolves: they review proposals, they teach the agent through PR feedback, they own the outcome. Throughput goes up 5–10×.

## 7. Continuous learning loop

Every customer interaction with Attest produces a learning signal:

- Triager auto-closure → if a human later reopens, that's a calibration signal.
- Investigator verdict → if SOAR confirms or refutes, that's a calibration signal.
- Detection Engineer PR → approval/rejection/edits become fine-tuning data.
- Responder action → outcome confirmed/refuted feeds the shadow-check policy refinement.

These signals power three loops:

1. **Calibration loop** — confidence calibration models are retrained nightly per agent per case-class.
2. **Detection mesh loop** — Detection Engineer agent's prompts and example library are updated weekly.
3. **Policy loop** — proposed policy changes (always human-approved) are surfaced to security leadership for review.

Customer data is never used to fine-tune base LLMs without explicit, contractual opt-in. Default learning loops use only metadata, structured outcome labels, and customer-controlled corpora.

## 8. The trust ladder

Customers progress along a maturity ladder, controlling how much autonomy they grant. Attest is designed to make moving up the ladder a deliberate, evidence-based choice — not a leap of faith.

- **Rung 1 — Observation.** Agents shadow human analysts. Every case has both a human and an agent verdict; agreement rates are tracked. No agent action is executed.
- **Rung 2 — Recommendation.** Agents propose actions; humans approve. Every approval/rejection feeds calibration.
- **Rung 3 — Bounded autonomy.** Agents auto-execute low-risk actions (auto-close benign Triager verdicts; auto-merge accepted Detection Engineer PRs).
- **Rung 4 — Supervised autonomy.** Agents auto-execute most response actions; humans review the audit trail asynchronously, with escalation triggers for outliers.
- **Rung 5 — Autonomy with policy.** Agents operate within a policy envelope; humans set policy and review attestations on demand.

Every customer enters at Rung 1. Movement up the ladder is an explicit configuration change with full audit trail.

## 9. Model strategy

- **Multi-provider by design.** Anthropic, OpenAI, Bedrock, Google, customer-self-hosted (Llama, Qwen). The agent runtime is provider-agnostic.
- **Bring-your-own-inference.** For cost-sensitive or sovereignty-sensitive customers, point Attest at their inference endpoint.
- **Per-task model selection.** Triager runs on fast/cheap; Investigator and Detection Engineer run on strong reasoning.
- **Open-weights fallback.** Reference architecture supports fully open-weights models for air-gapped or maximum-sovereignty deployments.

## 10. Evaluation framework

A platform that does not measure its own AI quality is unsafe. Attest ships with a continuous evaluation harness:

- **Golden-case evaluation.** A library of historical cases with ground-truth verdicts. Every model/prompt change is benchmarked against the golden set before deployment.
- **Red-team evaluation.** Library of adversarial cases (prompt injection in input data, indirect attacks, evasion patterns). Run on every release.
- **Regression evaluation.** Diff against last release on calibration, throughput, latency, cost.
- **Customer-specific evaluation.** Every customer has a small (~50 case) evaluation set built from their own ground-truth labels. Used to validate that updates do not regress on their specific environment.
- **Adversarial drift evaluation.** Specifically tests whether the platform's own agents become detectably worse under prompt-injection inputs.

Evaluation results are published to the customer in their workbench. Transparency is the differentiator.

## 11. AI governance posture

Security buyers, especially in regulated industries, increasingly require AI governance evidence:

- **NIST AI RMF** — Attest's agent governance maps to the Govern / Map / Measure / Manage functions; mapping document published.
- **ISO/IEC 42001** — AI management system certification roadmap committed for year 2.
- **EU AI Act** — agents in Attest are positioned as "high-risk AI systems" by default; conformity documentation provided.
- **HIPAA / HITRUST** — for healthcare deployments, the agentic plane runs in BYOC with PHI never leaving customer cloud.
- **Model governance reports** — per agent, per quarter: calibration, drift, policy adherence, incident summary.

## 12. The asymmetry we are creating

The competitive landscape today asks the customer to choose between:

- **Speed** (autonomous AI SOC, but trust the black box) — Microsoft, CrowdStrike, Prophet.
- **Control** (detection-as-code, but author by hand) — Panther, Datadog.
- **Composability** (streaming-first, but AI-on-top) — Abstract.
- **Open data** (lakehouse, but new agentic story) — Databricks Lakewatch.

Attest refuses the choice. The asymmetry we ship is:

> **Machine-speed defense + human-grade accountability + AI threat detection + continuous detection improvement, on a streaming-first composable substrate, with the customer's data in the customer's cloud.**

No single competitor has all of those at the same time today. The architectural decisions in this document are the path to having all of them in twelve months.
