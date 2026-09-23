# 11 — Repo Structure (high level)

**Product:** Attest
**Document type:** Top-level repository layout. Folders only. Read this when you sit down to `git init`.

> **Design document.** Describes the target layout. The actual tree may be smaller; directories are added when there is code to put in them.

---

## 1. Principle

A monorepo. Single Cargo workspace for all Rust crates. Side-by-side `apps/` and `packages/` directories for non-Rust code. All shared schemas, contracts, and content live in dedicated top-level folders so any service can consume them without circular dependencies.

This structure mirrors the architectural planes from `02_Architecture.md` and the build phases from `10_Build_Order.md` so that a new engineer joining can map any document to the corresponding folder in under a minute.

## 2. Top-level layout

```
Attest/
├── apps/                       # User-facing applications
├── crates/                     # All Rust services and libraries
├── ml/                         # Python ML training, calibration, evaluation
├── agents/                     # Agent definitions (system prompts, tool catalogs, configs)
├── detections/                 # HELIQL detection rules (versioned content)
├── schemas/                    # OCSF, attestation envelopes, HELIQL grammar
├── eval/                       # Golden cases, red-team corpora, evaluation harness inputs
├── infra/                      # IaC, Railway templates, Terraform modules, K8s manifests
├── tools/                      # Build scripts, codegen, dev tooling
├── docs/                       # All product/architecture/strategy markdown (this set)
├── tests/                      # Cross-cutting integration and chaos tests
├── examples/                   # Reference instrumented agent apps for AADF
├── .github/                    # CI workflows, PR templates, CODEOWNERS
├── docker-compose.yml          # Local dev stack (every service)
├── Makefile                    # `make seed-data`, `make dev-up`, `make smoke`
├── Cargo.toml                  # Workspace root
├── package.json                # Workspace root for JS/TS
└── README.md
```

## 3. `apps/` — user-facing applications

```
apps/
├── workbench/                  # Next.js (App Router) frontend — see 12_Workbench.md
│   ├── src/
│   │   ├── app/
│   │   │   ├── (marketing)/    # Public landing (route group)
│   │   │   ├── (auth)/         # /login, /sign-up
│   │   │   ├── workbench/      # Authenticated app shell
│   │   │   │   ├── queue/
│   │   │   │   ├── cases/[id]/
│   │   │   │   ├── hunt/
│   │   │   │   ├── detections/
│   │   │   │   ├── coverage/
│   │   │   │   └── agents/
│   │   │   ├── admin/
│   │   │   └── settings/
│   │   ├── components/
│   │   │   ├── ui/             # shadcn (generated)
│   │   │   ├── workbench/      # AlertQueue, CaseWorkbench, ReasoningTraceViewer, etc.
│   │   │   └── shared/
│   │   ├── lib/
│   │   │   ├── api/            # Typed clients for the Rust REST API
│   │   │   ├── ws/             # WebSocket subscription helpers
│   │   │   ├── posthog.ts      # PostHog init with strict masking
│   │   │   ├── sentry.ts       # Sentry init with customer-data scrubbing
│   │   │   └── env.ts          # Zod-validated env vars
│   │   ├── hooks/
│   │   ├── stores/             # Zustand
│   │   ├── schemas/            # Zod schemas (shared client/server-API)
│   │   └── styles/
│   └── e2e/                    # Playwright tests for workbench flows
├── workbench-api/              # Rust REST API for the workbench (axum)
└── ws-gateway/                 # Rust WebSocket gateway (live streams)
```

Refer to `12_Workbench.md` for the canonical workbench specification — stack, UI/UX principles, information architecture, the seven critical user flows, the fifteen non-trivial components, performance budget, and the PostHog + Sentry observability commitments with customer-data masking.

## 4. `crates/` — Rust workspace

Each is a Cargo crate; the workspace root ties them together.

```
crates/
├── attest-common/               # Shared types: OCSF events, IDs, errors
├── attest-collector/            # Edge collector binary (single static binary)
├── attest-ingester/              # Iceberg writer / Redpanda consumer
├── attest-stream-runtime/       # HELIQL → RisingWave / Arroyo compiler bridge
├── attest-detection-dsl/        # HELIQL parser, AST, type checker
├── attest-detection-compiler/   # HELIQL → SQL (RisingWave/Arroyo) + → Trino-compatible (federated)
├── attest-orchestrator/         # Agent runtime (Classifier / LLM / Hybrid execution paths)
├── attest-policy-engine/        # Deterministic per-agent action authorization
├── attest-mcp-gateway/          # MCP wire protocol; tool registry; intercept-and-log
├── attest-attestation/          # Envelope schema, Ed25519 signing, append-only log
├── attest-shadow-check/         # Deterministic shadow verification for high-impact actions
├── attest-inference-router/     # Routes LLM calls to Anthropic / OpenAI / Bedrock / vLLM / llama.cpp
├── attest-onnx-runtime/         # In-process ONNX classifier inference (Triager)
├── attest-storage-clickhouse/   # ClickHouse client + Iceberg integration
├── attest-storage-iceberg/      # Iceberg writer / reader
├── attest-control-plane/        # Identity, tenancy, audit, billing
├── attest-cli/                  # `attest verify` (signatures, hash chain, --pin-model/--pin-prompt) and `attest replay`
└── attest-test-utils/           # Shared test fixtures, assertion helpers
```

## 5. `ml/` — Python sidecar

```
ml/
├── triager_classifier/         # XGBoost training, ONNX export, SHAP explainer
├── novelty_detector/           # Mahalanobis / Isolation Forest OOD detector
├── calibration/                # Isotonic regression per agent / path / case-class
├── sequence_anomaly/           # LSTM autoencoder (per-customer fine-tunes)
├── network_anomaly/            # XGBoost on flow features
├── prompt_injection/           # Wrapper around pretrained DeBERTa classifier
├── embeddings/                 # Wrapper around BGE / similar pretrained embedding
├── eval_runner/                # Runs golden cases, red-team, customer corpora
├── feature_extractor/          # Alert → feature vector (used by Rust orchestrator via gRPC)
└── sidecar/                    # FastAPI service exposing all of the above
```

## 6. `agents/` — agent definitions

Versioned, signed, immutable. Each subfolder contains the system prompt, tool catalog, model config, policy scope, and metadata for one agent.

```
agents/
├── coordinator/                # Planner: classifier execution path
├── triager/                    # Hybrid: classifier primary, LLM escalation
│   ├── classifier/             # XGBoost artifact, feature spec
│   └── llm/                    # System prompt, tool catalog, model config
├── investigator/               # LLM execution path
├── hunter/                     # LLM execution path
├── detection_engineer/         # LLM execution path
└── responder/                  # LLM execution path (constrained, low-temperature)
```

## 7. `detections/` — HELIQL content

```
detections/
├── identity/                   # Authentication, IdP, MFA detections
├── cloud/                      # AWS, Azure, GCP control-plane detections
├── endpoint/                   # EDR-derived detections
├── network/                    # Flow / firewall / DNS detections
├── saas/                       # M365, Okta, Workday, Salesforce, etc.
├── aadf/                       # OWASP LLM Top 10 detections (open-sourced)
├── deprecated/                 # Retired detections kept for replay
└── shared/                     # Reusable HELIQL macros and entity definitions
```

## 8. `schemas/` — contracts

```
schemas/
├── ocsf/                       # OCSF JSON schemas + Attest extensions (e.g., AIAgent entity)
├── attestation/                # Envelope JSON schema (Classifier / LLM / Hybrid variants)
├── heliql/                     # HELIQL grammar (.pest or .lalrpop) + reference docs
├── otel-genai/                 # OpenTelemetry GenAI semantic conventions (vendored)
├── mcp/                        # MCP wire protocol schemas
└── policy/                     # Policy-as-code typed DSL schema (GA v1)
```

## 9. `eval/` — evaluation corpora

```
eval/
├── golden_cases/               # 100 hand-curated cases with ground-truth verdicts
├── red_team/                   # AgentDojo, InjecAgent, internal red-team scenarios
├── regression/                 # Customer-reported false-positives/negatives → permanent tests
├── per_customer/               # Per-design-partner ~50-case eval sets (gitignored, encrypted)
├── synthetic/                  # Generated agent traces from `examples/` reference apps
└── reports/                    # CI-generated evaluation reports per release
```

## 10. `infra/` — deployment

```
infra/
├── railway/                    # Railway service definitions and env templates
├── terraform/                  # BYOC modules: AWS / GCP / Azure
├── helm/                       # K8s charts (for self-hosted / air-gapped reference)
└── docker/                     # Production Dockerfiles (per service)
```

## 11. `tools/` — dev tooling

```
tools/
├── seed-data/                  # Downloads Tier 1 datasets to MinIO; `make seed-data`
├── ocsf-codegen/               # Generates Rust types from OCSF JSON schemas
├── load-gen/                   # Generates synthetic alert volume for load tests
└── red-team-runner/            # Triggers AgentDojo / InjecAgent scenarios against reference agents
```

## 12. `docs/` — this document set

```
docs/
├── README.md                   # Index and reading paths
├── 01_PRD.md
├── 02_Architecture.md
├── 03_AI_Agentic_Strategy.md
├── 03_Architecture_Diagrams.md
├── 07_Stack_Revised.md
├── 08_Datasets_and_ML.md
├── 09_Agent_Harness.md
├── 10_Build_Order.md
├── 11_Repo_Structure.md        # This document
├── 12_Workbench.md             # Workbench stack, UI/UX, flows, observability
├── 15_Streaming_Engine_Decision.md  # ADR: Arroyo vs. Flink vs. RisingWave (new ADRs sit alongside it)
├── phases.md                   # What actually shipped, phase by phase
└── runbooks/                   # On-call runbooks (planned)
```

## 13. `tests/` — cross-cutting integration

Per-crate unit tests live inside each crate (Rust convention) and per-module tests live inside each Python module. This folder is for tests that span multiple services and that exercise the full docker-compose stack.

```
tests/
├── e2e/                        # End-to-end tests from `10_Build_Order.md` (Phase 1–11)
├── load/                       # Sustained 1K eps for 1 hr; 100 concurrent agents
├── chaos/                      # Kill Redpanda mid-stream; partition orchestrator from MCP gateway
├── compliance/                 # PII redaction, tenant isolation, append-only attestation
└── playwright/                 # Workbench UI E2E
```

## 14. `examples/` — reference instrumented agent apps

These are the four reference agents from `08_Datasets_and_ML.md` Section 6.1. They are deployed to the local docker-compose for AADF testing and to Railway for design-partner demos.

```
examples/
├── customer-support-agent/     # Email + CRM tools; LLM01 indirect-injection scenarios
├── devops-agent/               # Shell + GitHub + cloud-console MCP; LLM06 tool-misuse
├── finance-agent/              # RAG over financial documents; LLM02 sensitive disclosure
└── multi-agent-research/       # A2A coordination; LLM06 + A2A smuggling scenarios
```

## 15. `.github/` — GitHub-specific

```
.github/
├── workflows/                  # CI: fmt, clippy, cargo test, pytest, e2e via docker-compose
├── ISSUE_TEMPLATE/
├── pull_request_template.md    # Required: E2E test, eval result, ADR if architectural
└── CODEOWNERS                  # Per-folder ownership; `agents/` requires senior detection eng review
```

## 16. The five things to verify before `git init`

A final checklist a new contributor should be able to run in 10 minutes:

1. **`docker-compose up`** brings up the entire stack on a laptop in <5 min: Redpanda, RisingWave, Arroyo, ClickHouse, MinIO, Postgres, llama.cpp+Qwen, Python ML sidecar.
2. **`make seed-data`** downloads Tier 1 datasets into MinIO and Postgres in <5 min, cached for re-runs.
3. **`make dev-up && make smoke`** runs the Phase 0 smoke test: stack is healthy, every service responds.
4. **`cargo test --workspace`** + **`pytest ml/`** + **`pnpm test`** all run green on a fresh checkout.
5. **`cargo run --bin Attest-cli -- run-case --case sample-credential-stuffing`** runs the full Triager → Investigator loop locally on Qwen 3 7B and produces a signed attestation envelope on disk.

Hitting all five within the first day of onboarding is the bar for "the project structure is real."

## 17. What is intentionally not in the repo

For clarity:

- **No customer data.** Per-customer eval sets are encrypted and access-controlled separately.
- **No model weights.** Anthropic API keys and Qwen GGUF files are pulled at build time from secure caches.
- **No detection content under proprietary license.** Sigma rules in `detections/` are MIT-licensed; customer-specific extensions live in customer repos.
- **No secrets.** Every deployment manifest reads from a secrets manager (Railway env vars, AWS Secrets Manager, Terraform Cloud / HCP Vault).
- **No `agents/.../classifier/` model artifacts in Git LFS for the production version.** Models are built in CI, signed, and published to a private registry; only the training code lives in Git.
