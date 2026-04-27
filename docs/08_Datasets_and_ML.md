# 08 — Datasets & ML Strategy

**Product:** Attest
**Document type:** What data we use to develop, test, and demo. What ML models we use, where we train, and where we don't. Read this before any ML engineer starts work.

---

## 1. The honest framing

The biggest mistake a security-product team can make on ML is to assume "we need to train models." The second-biggest mistake is the inverse: assuming foundation LLMs are the answer to every problem. The truth is in the middle. In 2026, **the right defaults are:**

- **Use pretrained foundation models for open-ended reasoning** (deep investigation, hunting, detection authoring, NL→code translation). LLMs do this work; we don't train our own LLM.
- **Train small targeted classifiers for high-volume classification** where the data is tabular and the volume is too high for LLM cost or latency. The Triager XGBoost classifier is the canonical case — ~80% of Attest's alert volume.
- **Use lightweight statistical methods for behavioral baselines.** Per-entity baselines are not deep learning problems; robust statistics outperform deep methods on real security data.
- **Use targeted, small, supervised models** where the open-source pretrained ones are insufficient — and accept that those cases exist (sequence anomaly autoencoders, network anomaly XGBoost).
- **Train calibration models, always.** Per-agent, per-execution-path, per-case-class isotonic regressions. These cover both classifier outputs and LLM self-reported confidence.

This document is organized to enforce that discipline: **ML where it earns its place, not ML for marketing, and not LLM-for-everything either.** The latter is the more common mistake in 2026 agentic-SOC products and is one of Attest's central competitive positions to avoid.

## 2. Datasets — the realistic landscape

The single biggest accelerator for Attest development is having a credible, large, labeled corpus of security telemetry from day one. We do not have to build this from scratch. The security ML community has assembled excellent public datasets; the question is which to use for what purpose.

### 2.1 Tier 1 — must-have datasets for MVP development

| Dataset | What it is | What we use it for | Size | License |
|---|---|---|---|---|
| **OCSF Sample Data** | Reference events in OCSF format covering authentication, network activity, file system, process activity | Validating OCSF normalization, ingest pipeline, schema evolution tests | ~100 MB | Apache 2.0 |
| **AWS CloudTrail public sample logs** | Real-shape CloudTrail events from AWS docs and labs | Connector validation, detection authoring against realistic AWS traffic | varies | Public |
| **Microsoft 365 audit logs (synthetic)** | Microsoft Sentinel notebook examples + Defender XDR sample data | M365 / Entra detection authoring; sample EDR traces | ~500 MB | Microsoft sample license |
| **MITRE ATT&CK Evals public data** | Telemetry from MITRE's annual ATT&CK evaluations of EDR vendors | Coverage validation; the gold standard for "does Attest detect what real attackers do" | ~10 GB | MITRE / CC BY 4.0 |
| **CICIDS 2017/2018 + UNSW-NB15** | Labeled network intrusion datasets, widely cited | Network UEBA baselines, anomaly detection model training | 50–100 GB | Free for research |
| **DARPA OpTC** | Operationally Transparent Cyber dataset — host telemetry from a 1000-host network with labeled red-team activity | Host UEBA, lateral movement detection, end-to-end multi-stage attack scenarios | ~1.5 TB | US Gov public release |

### 2.2 Tier 2 — strongly recommended for AADF and Verifiable Agentic SOC work

These are the datasets nobody else has integrated yet, and they are critical to the AADF differentiator.

| Dataset | What it is | What we use it for | Source |
|---|---|---|---|
| **AgentDojo** | Adversarial benchmark for evaluating LLM agents — 97 tasks, 629 attacks across email, banking, slack, travel agents | Red-team eval corpus for AADF; evaluating whether Attest's own agents are robust to prompt injection | github.com/ethz-spylab/agentdojo |
| **AgentHarm** | Benchmark for measuring harmfulness of LLM agents | Red-team for Responder agent guardrails | github.com/dsbowen/agentharm |
| **InjecAgent** | Indirect prompt injection benchmark for tool-using agents | AADF detection content validation for LLM01 indirect injection | github.com/uiuc-kang-lab/InjecAgent |
| **PromptBench / advbench** | Prompt-injection adversarial test suites | LLM01 direct injection detection content |  |
| **MS MARCO + BEIR** | Retrieval benchmarks | Vector store / RAG retrieval anomaly baselines (LLM08) | Public |
| **OpenAI / Anthropic safety eval suites** (publicly released portions) | Refusal, jailbreak, harmful content evaluations | General agent robustness | Vendor-published |
| **OWASP Top 10 for LLM examples repo** | Reference exploits for each LLM Top 10 category | AADF detection content seeds | github.com/OWASP/www-project-top-10-for-large-language-model-applications |

### 2.3 Tier 3 — synthesized data we generate

Even with the public datasets above, we will need to generate data for three specific scenarios where public corpora are weak:

1. **Multi-agent A2A traces.** Almost no public dataset captures realistic A2A protocol traffic. We generate synthetic scenarios using Anthropic's MCP and a multi-agent framework (LangGraph or CrewAI) running scripted enterprise tasks (customer support, internal IT, document review) with red-team injections at known points.
2. **OTEL GenAI traces from realistic agent applications.** We instrument 4–5 reference agent apps (a customer-support agent, a DevOps copilot, a finance assistant, a research assistant) and run them against scripted traffic, with periodic red-team injection.
3. **Customer-shaped detection backtesting data.** During design-partner onboarding, the partner provides 30 days of their own production telemetry (anonymized, contractually scoped) for backtest. This is the single most valuable training signal for the calibration loop.

### 2.4 Detection content corpora

| Corpus | What it is | Use |
|---|---|---|
| **Sigma rules repo** | ~3,000 community-maintained detection rules in Sigma format | Seed Attest's detection content; validate Sigma compatibility |
| **MITRE ATT&CK technique definitions** | The structured knowledge graph of techniques, tactics, sub-techniques | Coverage map; Detection Engineer agent retrieval corpus |
| **Atomic Red Team** | Library of executable test procedures mapped to ATT&CK | Validation of detection content against known attack procedures |
| **CISA known-exploited vulnerabilities feed** | Real-time feed of actively exploited CVEs | Threat intel ingestion for the Detection Engineer agent's "what's new" loop |
| **AlienVault OTX, MISP feeds** | Community threat intel | IOC enrichment in the streaming pipeline |

### 2.5 Local development corpus — what every engineer's laptop should have

A curated 5–10 GB subset that fits on a developer laptop and exercises every code path. We commit a `make seed-data` target that downloads and shapes:

- 100 MB OCSF sample (control corpus, schema validation).
- 1 GB CICIDS network traffic (UEBA baseline).
- 2 GB DARPA OpTC subset, one specific red-team scenario (end-to-end detection).
- 100 MB AgentDojo + InjecAgent (AADF red-team).
- 500 MB synthesized agent traces from our reference agent apps.
- Sigma rules repo (clone, ~50 MB).
- MITRE ATT&CK STIX bundle (~10 MB).

This makes local CI runnable in <5 minutes against realistic data.

## 3. ML model strategy by use case

### 3.1 The Triager — a hybrid XGBoost + LLM design (the most consequential ML decision in MVP)

**Approach:** Classifier first, LLM escalation. The Triager is **not** an LLM by default. It is a calibrated XGBoost classifier that routes the well-known ~80% of alerts in milliseconds, with an LLM tail for the novel/ambiguous remainder.

This is a deliberate departure from the agentic-SOC marketing default ("LLMs everywhere") and is one of Attest's core competitive positions. The reasoning is fully laid out in `09_Agent_Harness.md` Section 2.5; this section covers the ML specifics.

#### 3.1.1 The classifier

| Item | Choice | Rationale |
|---|---|---|
| Model | **XGBoost** (gradient-boosted trees) | Empirically validated state of the art on tabular security data; Mohamed has prior production experience |
| Inputs | Alert features: severity, source, entity reputation, baseline deviation, threat intel hits, time of day, asset criticality, recent context, prior dispositions on similar alerts | All computed in the streaming substrate; no pre-existing model dependency |
| Output | Calibrated probability per disposition class (`true_positive`, `false_positive`, `needs_investigation`) + per-feature SHAP attribution |  |
| Training data | Golden case corpus + DARPA OpTC labeled scenarios + AgentDojo + design-partner accumulated dispositions | See Section 2 of this doc |
| Inference latency | <10ms on CPU | Single ONNX prediction in the orchestrator process |
| Serving | ONNX-serialized; loaded by the Rust orchestrator via ONNX Runtime; no separate service hop | See `09_Agent_Harness.md` Section 4.1 |

**Why XGBoost not a neural net:**
- Tabular features. NNs do not outperform GBMs on this data shape; published benchmarks are unambiguous.
- Native SHAP feature attribution. Auditors and regulated buyers find this far more interpretable than neural attention maps.
- Cheap to train (minutes on CPU); cheap to retrain monthly per customer.
- Reliable cold-start: a generic model trained on the public corpora ships with Attest from day one and is fine-tuned per customer as their data accumulates.

#### 3.1.2 The escalation criteria

The classifier escalates to the LLM path when *any* of:
- Calibrated confidence falls below the per-tenant configured threshold (default 0.85).
- Out-of-distribution score on the input feature vector exceeds threshold (the alert looks unlike anything in the training corpus).
- The disposition the classifier proposes is `needs_investigation` (a category that means "I'm not certain; a deeper look is needed").

The escalation is logged in the attestation envelope with structured reasons, so customers can audit *why* a particular alert triggered LLM judgment.

#### 3.1.3 The LLM escalation path

For escalated alerts, the Triager invokes Claude Sonnet (production) or Qwen 3 7B (local/air-gapped). This is the LLM execution path described in `09_Agent_Harness.md` Section 4.1.3. Latency: 8–15 seconds. Cost: ~$0.005/case. Volume: ~20% of total triage volume.

**The economic and latency math:**
- Without hybrid: 100% of alerts × 8–15s = 100% slow, 100% expensive.
- With hybrid: 80% of alerts × 10ms + 20% × 8–15s = ~98% reduction in median latency, ~80% reduction in LLM cost.

#### 3.1.4 What we do, what we don't

**What we build:**
- The XGBoost classifier with a versioned feature extractor.
- A novelty detector (Mahalanobis distance or isolation forest) on the input feature vector for OOD detection.
- Per-tenant fine-tuning pipeline that retrains monthly on accumulated dispositions.

**What we don't build:**
- A custom LLM. We use Claude/Qwen for the escalation path.
- A neural Triager. Tabular data, GBMs win.

### 3.2 Investigator / Hunter / Detection Engineer reasoning

**Approach:** Use foundation LLMs. Do not train.

**Models:**
- Production: Claude Opus (Investigator, Hunter, Detection Engineer).
- Local dev / air-gapped: Qwen 3 32B (acknowledging quality gap vs. Claude Opus, and disclosing it to the customer).

**Why not train:**
- The capability gap between fine-tuned 7B–32B models and frontier models on multi-step reasoning is wider than any Attest-internal training corpus could close.
- Training is a 6-month detour from shipping the platform.
- The differentiator is the harness around the LLM (attestation, calibration, shadow checks), not the LLM itself.

**Why these roles use LLMs (and the Triager doesn't by default):**
- Investigation is open-ended multi-step reasoning across heterogeneous data sources. There is no good non-LLM substitute.
- Hunting is creative hypothesis generation. Same.
- Detection authoring is natural-language → code translation. LLMs are uniquely good at it; humans take hours.
- These workloads are low-volume (10–50× less than Triage volume) and tolerate higher latency. LLM cost and latency budget are not the binding constraint.

**What we do build:** prompt libraries with versioning, few-shot exemplar libraries that grow with customer deployments, a system-prompt evaluation harness (covered in `09_Agent_Harness.md`).

### 3.3 Behavioral baselines (UEBA primitives)

**Approach:** Robust statistics + simple ML, in this order of preference.

**Per-entity baseline computation:**

| Signal class | Method | Why this method |
|---|---|---|
| Login locations / countries | Set membership + decay weight | Categorical, not numerical |
| Login times of day | Circular statistics (von Mises distribution) | Hours wrap; Gaussian is wrong |
| Volume metrics (bytes, requests, calls) | Robust z-score (median + MAD) | Robust to outliers and warm-up |
| Sequences of actions | Markov model with smoothing | Lightweight, interpretable, fast |
| Tool-call distributions for AI agents | Categorical distribution + Jensen-Shannon divergence drift | Aligned to agent baseline question |
| Inter-event timing | Log-normal or empirical CDF | Heavy-tailed, gracefully handled |

**These run in RisingWave as continuous materialized views.** No GPU needed. Sub-second latency. Orders of magnitude cheaper than deep-learning UEBA, and **more interpretable**, which matters for the verifiability story.

**Where deep learning earns its place — and where it doesn't:**
- ✅ **Sequence anomaly detection in extreme high-volume cases** (e.g., process trees on 10K+ hosts) — autoencoder-based anomaly score; see Section 3.5.
- ❌ **General "AI for UEBA"** — hype-driven; the academic literature consistently shows simple methods within 5% of deep methods on real security data, with 100× lower compute and infinitely better interpretability.

### 3.4 Confidence calibration

**Approach:** Isotonic regression, per-agent, per-case-class. Retrained nightly.

**Inputs:** raw LLM self-reported confidence (0–1), case-class label (e.g., "credential-stuffing-aws").
**Output:** calibrated probability of correctness.
**Training data:** historical agent decisions paired with ground-truth outcomes (analyst review, SOAR-confirmed, customer feedback).
**Implementation:** scikit-learn `IsotonicRegression`, ~50 lines of Python, in the ML sidecar.

**Why isotonic and not a neural net:**
- Few thousand to few hundred-thousand observations per agent per case-class. Too small for neural calibration.
- Monotonic constraint matches reality (higher self-reported should map to higher calibrated, ceteris paribus).
- Interpretable: the calibration curve is publishable to the customer.
- Cheap: nightly retrain in seconds.

**Metrics tracked per agent per case-class:**
- Brier score (lower is better).
- Expected Calibration Error (ECE).
- Reliability diagram (visualization).

These are published in the workbench. Customers see the actual calibration of Attest's own AI.

### 3.5 Prompt injection / OWASP LLM Top 10 detection

**Approach:** Hybrid — fast rule-based filter first, then ML classifier.

**Tier 1 — fast rules (run in stream):**
- Pattern matching for known injection signatures (system prompt overrides, role-confusion patterns, unicode tricks).
- Heuristics for content from low-trust sources (web pages, emails) appearing in agent inputs.
- Statistical anomalies on prompt structure (length, nested instructions, language switches).

These catch maybe 60–70% of attacks; cheap and run inline.

**Tier 2 — ML classifier (run async):**
- A pretrained prompt-injection classifier such as **`protectai/deberta-v3-base-prompt-injection-v2`** (or successor) — DeBERTa-v3 fine-tuned on prompt-injection corpora. Inference cost is ~10ms per request on CPU.
- This is a **pretrained model we serve, not train.** The HuggingFace ecosystem has several maintained prompt-injection classifiers; we evaluate and pick the best on our internal eval set quarterly.

**Tier 3 — LLM-as-judge for ambiguous cases:**
- For cases the rules and classifier flag with low confidence, we use a fast LLM call (Claude Haiku) to evaluate the input against a structured rubric.
- Adds latency but only for the ambiguous tail.

**What we don't do:** train our own prompt-injection model from scratch. The public corpora and pretrained models are good enough; our edge is the *integration into a SIEM*, not the classifier itself.

### 3.6 Sequence anomaly detection (when robust statistics aren't enough)

**Approach:** Pretrained autoencoder for sequences, fine-tuned per-customer on 30-day baselines.

**When we use it:** for high-volume, high-cardinality sequences where simple Markov models miss subtle patterns. Example: process-execution sequences on a Linux host. Robust stats catch 80%; an autoencoder catches the next 10–15%.

**Implementation:**
- A small LSTM autoencoder, ~100K parameters, runs in the ML sidecar (PyTorch → ONNX → ONNX Runtime in the Rust hot path).
- Per-customer fine-tuning on 30 days of their data.
- Reconstruction error above per-entity threshold → anomaly signal.

**Why this is a real ML investment and not theater:** the payoff is incremental detection coverage on attacks that mimic legitimate activity, which is precisely the AI-agent threat model. The cost is bounded (a small model trained per customer in the cloud).

### 3.7 Network anomaly detection

**Approach:** Off-the-shelf gradient-boosted tree on flow features.

- Features: standard NetFlow / Zeek-style flow statistics.
- Model: XGBoost (Mohamed has prior production experience here).
- Training data: CICIDS + UNSW-NB15 + customer-specific 30-day baseline.
- Output: anomaly score per flow, fed into the streaming detection pipeline.

**No deep learning here.** XGBoost on flow features is the empirically validated state of the art for general network anomaly; deep models marginally improve and dramatically increase cost/complexity.

### 3.8 Embedding for semantic search and case similarity

**Approach:** Pretrained embeddings; do not train.

- Model: a strong open-source embedding model (e.g., `BAAI/bge-large-en-v1.5` or successor). Evaluate on MTEB leaderboards quarterly; swap as the field moves.
- Use cases:
  - Semantic search across past cases for the Investigator agent's RAG step.
  - Detection content similarity (find duplicates, near-duplicates).
  - Threat-intel description embedding for IoC matching.
- Storage: a pgvector extension on the control-plane Postgres handles MVP volumes; consider Qdrant or LanceDB if vector volume grows past ~10M vectors.

### 3.9 Detection content quality scoring

**Approach:** Statistical, not ML.

For each detection in production, we compute:
- Precision over last 7/30 days (true positives / total fires).
- Recall estimate (joint with the SIDM coverage map).
- Drift score (KL-divergence of fire distribution week-over-week).
- Cost (compute resource consumption per fire).

A composite "detection health score" surfaces low-yield rules to the Detection Engineer agent for retirement proposals. This is plain weighted scoring, not ML — interpretability is the priority.

## 5. Models we train ourselves — and when

The honest inventory of what Attest trains, contradicting the original framing of this document.

### 5.1 Trained at MVP (Day 0–90)

**The Triager XGBoost classifier (per-tenant cold-start version).** This is the single most consequential ML deliverable in MVP. A generic classifier trained on the public corpora (golden cases + DARPA OpTC + AgentDojo + synthetic) ships with Attest from day one. Every customer starts with this generic model. Specifications in Section 3.1.

**Confidence calibration models** — isotonic regressions, per-agent, per-execution-path, per-case-class. Retrained nightly. Specifications in Section 3.4.

**Per-customer behavioral baselines** — robust statistical estimators running continuously in RisingWave. Not "training" in the deep-learning sense; live online estimation. Specifications in Section 3.3.

**Generic XGBoost network anomaly model** — trained on CICIDS + UNSW-NB15 baseline; ships with Attest. Specifications in Section 3.7.

### 5.2 Fine-tuned per-customer at GA v1 (Day 90–180)

**Per-customer Triager classifier fine-tunes.** As each customer accumulates 30+ days of Triager dispositions, we retrain the XGBoost on their data. This is fast (minutes on CPU) and substantially improves precision on customer-specific noise patterns.

**Per-customer LSTM autoencoder for sequence anomaly** — fine-tuned on each customer's 30-day baseline of high-volume sequence data (process trees, syscall sequences). Specifications in Section 3.6.

**Per-customer XGBoost network anomaly fine-tunes** — when the customer has enough labeled flow data.

### 5.3 Considered for Year 2 (Day 180–730)

**Per-customer LoRA fine-tunes of Qwen 3 for the Investigator escalation path.** Rationale: as customers accumulate 6+ months of Investigator decisions paired with outcomes, the data may be enough to materially improve a 32B open-weights model toward Claude Opus quality on that customer's specific case patterns. This is a sovereignty play (air-gapped customers cannot use Anthropic API) more than a quality play.

**This is explicitly a Year 2 investment, not earlier.** It depends on accumulated data we don't have at MVP.

### 5.4 What we explicitly never train

| Tempting to train | Why not | What we do instead |
|---|---|---|
| Our own foundation LLM | 18-month detour; can't beat frontier models | Use Claude / Qwen with great prompts and retrieval |
| Custom prompt-injection classifier | Public pretrained models are competitive | Use DeBERTa-prompt-injection variants |
| Custom UEBA neural network | Robust stats outperform on real data | Median+MAD, von Mises, Markov models |
| Custom embedding model | MTEB leaderboard improves monthly | Use BGE / similar pretrained |
| Custom MITRE technique classifier | LLM with retrieval over MITRE STIX is better | RAG into Detection Engineer agent |
| Custom detection-rule generator | LLM with backtest harness is better | Detection Engineer agent + backtest |
| Neural Triager | Tabular features; GBMs win unambiguously | XGBoost with SHAP attribution |

The pattern: train **small, interpretable, customer-specific models on tabular data** where the training is cheap and the differentiation is real. Buy off-the-shelf for everything else.

## 6. Synthetic data generation strategy

For three categories where public datasets are weak, we generate synthetic data ourselves. The principles:

1. **Generated, not made up.** We run real agent applications instrumented with real OTEL GenAI tracing. The traces are real; only the scenarios are scripted.
2. **Adversarial pairing.** Every benign scenario has at least one matched adversarial variant (with prompt injection, with tool misuse, with A2A smuggling).
3. **Versioned and signed.** Synthetic corpora are versioned in a Git LFS repo with provenance metadata; we never confuse generated data with customer data.
4. **Public release where possible.** The OWASP LLM Top 10 detection content + matched red-team scenarios are released as a public benchmark; this seeds the AADF category and demonstrates technical credibility.

### 6.1 Reference agent apps we instrument

Pick four reference agents that span the realistic enterprise threat model:

1. **Internal customer-support agent** with email and CRM access — exposed to indirect prompt injection via emails.
2. **DevOps assistant** with shell, GitHub, and cloud-console MCP servers — exposed to tool misuse and supply-chain attacks.
3. **Finance assistant** with read access to a vector store of financial documents — exposed to RAG poisoning and sensitive disclosure (LLM02).
4. **Multi-agent research team** — coordinator dispatches to web-research, code-execution, and writing specialists — exposed to A2A smuggling.

Each runs in a sandboxed Railway environment producing OTEL GenAI traces, MCP traces, A2A traces, output logs, and underlying classical telemetry (network, identity). We record both clean runs and red-team-injected runs.

## 7. Evaluation framework — the data side

Models without continuous evaluation are unsafe. The evaluation corpus structure:

- **Golden cases (100 cases minimum at MVP):** historical investigations with ground-truth verdicts, used to benchmark every prompt or model change.
- **Red-team adversarial corpus (300+ cases):** prompt injection, indirect injection, tool misuse, A2A smuggling, RAG poisoning. Run on every release.
- **Regression corpus:** automatic capture of every customer-reported incorrect verdict. Becomes a permanent test case.
- **Customer-specific corpus (per design partner, ~50 cases):** built from their ground-truth labels during onboarding. Validates that updates don't regress on their environment.
- **Drift corpus:** synthetic prompt-injection variants generated weekly by an LLM red-team agent, kept fresh against evolving attacker techniques.

The agent harness (`09_Agent_Harness.md`) is the runtime that exercises these corpora. This document covers what's *in* the corpora.

## 8. Practical answers to your specific questions

> *"What ML models do we need to use for these and do we need to train any models?"*

**Train at MVP (small but mission-critical):**
- **Triager XGBoost classifier** — generic version trained on public corpora at MVP; per-customer fine-tunes from GA v1. This is the load-bearing ML deliverable; ~80% of alert volume flows through it.
- **Confidence calibration models** — isotonic regressions per agent per execution path per case-class, nightly retrain.
- **Generic XGBoost network anomaly model** — trained on CICIDS + UNSW-NB15.
- **Per-customer behavioral baselines** — online statistical estimators in RisingWave, not "training" in the deep-learning sense.

**Use, don't train:**
- LLMs: Claude (cloud) for production, Qwen 3 (local/air-gapped). Used for Investigator, Hunter, Detection Engineer, and the Triager LLM escalation tail.
- Prompt-injection classifier: pretrained DeBERTa variant from HuggingFace.
- Embeddings: BAAI/bge-large-en-v1.5.
- MITRE / threat intel: structured knowledge graph + retrieval, no ML.

**Train if specific need (per-customer, GA v1+):**
- LSTM autoencoder for sequence anomaly on high-volume process logs.
- Per-customer LSTM fine-tunes when the customer has 30+ days of data.

**Train Year 2:**
- Per-customer LoRA fine-tunes of Qwen 3 for the Investigator escalation path (sovereignty deployments).

> *"Which models work best in this domain — in real world and also with the new AI agentic world?"*

**Real-world security operations (today's threats):**
- For high-volume triage classification: **XGBoost on tabular alert features**. State of the art on this data shape; native SHAP attribution; sub-10ms inference.
- For deep multi-step investigation reasoning: **Claude Opus** is empirically the strongest on community benchmarks and our internal evaluation; Qwen 3 32B is the best open-weights option for sovereignty constraints.
- For embeddings: **BGE family** is consistently top of MTEB; specific variants change quarterly.
- For prompt-injection classification: **fine-tuned DeBERTa-v3** (the ProtectAI line) is the established baseline; expect a successor by mid-2026.
- For network anomaly: **XGBoost on flow features** remains the empirically validated standard.
- For UEBA baselines: **robust statistics** (von Mises for time-of-day, MAD for volumes, Markov for sequences) — published research consistently shows these match or beat deep methods on real security data.

**AI agentic world (2026 threats specifically):**
- For agent reasoning trace analysis: same LLMs as above (Investigator path), with retrieval over the OWASP LLM Top 10 knowledge base.
- For detecting prompt injection in agent inputs: the DeBERTa prompt-injection classifier as the primary path + LLM-as-judge (Claude Haiku) for ambiguous cases. Note this is structurally identical to the Triager hybrid: classifier first, LLM tail.
- For agent behavior baselining: per-agent Markov + categorical-distribution drift (Jensen-Shannon divergence) — same toolkit as classical UEBA, applied to a new entity type. **This is the unique Attest positioning: the Agent-Aware Detection Fabric uses well-understood statistical methods on a new entity class, which is why we can ship AADF in MVP rather than wait for novel ML breakthroughs.**

> *"Why do we need LLMs for this — wouldn't it make it slower?"*

**Excellent question; the honest answer:** ~70% of Attest runs without an LLM in the path at all. ~20% genuinely needs an LLM (deep investigation, hunting, detection authoring, NL→code translation). ~10% used to be ambiguous, but the resolution is the hybrid Triager: classifier-first by default, LLM only when the classifier flags low confidence or out-of-distribution.

The latency math:
- Stream detection: <100ms (RisingWave SQL). LLM here would add 5–30s and provide no benefit.
- Triager (80% of volume): <50ms (XGBoost + SHAP + calibration). LLM here would be 100–1000× slower with no quality gain.
- Triager (20% escalation): 8–15s (Claude Sonnet). LLM here is the right tool.
- Investigator: 30s–3min (Claude Opus). LLM is the only viable option.
- Hunter / Detection Engineer: 30s–2min. Same.

Compared to deterministic code paths, LLMs are slow. Compared to human SOC analysts on the same workload, LLMs are 100–1000× **faster**. The whole point of agentic SOC is replacing human-paced work with machine-paced work; that comparison is what justifies LLM latency where we use it.

This honest analysis is also a competitive advantage. Most agentic-SOC competitors pitch "LLMs everywhere" and incur the latency and cost without admitting it. Attest pitches "LLMs where they earn it" — measurably faster, cheaper, and more verifiable.

## 9. Build vs. buy for ML capabilities

| Capability | Decision | Rationale |
|---|---|---|
| LLM inference | **Buy** (Anthropic + open-weights via vLLM) | Frontier capability, multi-provider |
| **Triager XGBoost classifier** | **Build** (XGBoost on alert features, SHAP attribution) | **Core IP. The hot path of the entire platform.** |
| Prompt-injection classifier | **Buy** (pretrained from HuggingFace) | Mature, cheap |
| Embedding model | **Buy** (pretrained, swap as MTEB updates) | Mature, cheap |
| Threat-intel ingestion | **Buy** (MISP, STIX/TAXII feeds) | Standard |
| MITRE ATT&CK knowledge | **Buy** (MITRE-published STIX bundle) | Authoritative |
| UEBA statistical methods | **Build** (run in RisingWave) | Trivial code, integrated with the stream |
| Calibration models | **Build** (scikit-learn isotonic regression) | Trivial code, mission-critical, covers both paths |
| Sequence autoencoder | **Build** when needed | Small custom model per customer |
| XGBoost network anomaly | **Build** (standard pipeline) | Standard, reproducible |
| Vector store | **Buy** (pgvector → Qdrant if needed) | Standard |

**Pattern:** the ML investments are the small custom things tightly coupled to Attest's product (Triager classifier, calibration, baselines, customer fine-tunes). Everything else is a foundation model or pretrained component used off the shelf.

## 10. Data engineering responsibilities (where the actual work is)

The team time you'll spend on "ML" will overwhelmingly be data engineering, not model training:

- **Day 1–30:** download and shape Tier 1 datasets; commit `make seed-data`.
- **Day 30–60:** build the synthetic agent traces; instrument the four reference apps.
- **Day 60–90:** wire the calibration loop end-to-end (decisions → outcomes → nightly retrain).
- **Day 90–180:** customer-specific baselines and fine-tunes; per-customer eval corpora.
- **Day 180+:** continuous improvement on red-team corpus; quarterly evaluation reports.

The ML engineer on the founding team spends ~70% of time on data pipelines, ~20% on calibration and statistical baselines, ~10% on actual model training. That ratio is correct for Attest's product.

## 11. The decision principle, distilled

> **For Attest, ML is a tool used in service of the harness, not the product. The product is the verifiable agentic SOC; the ML work is whatever is necessary to make agents calibrated, baselines accurate, and detections precise — and nothing more.**

Holding that line saves a year of effort and ships the product the market actually needs.
