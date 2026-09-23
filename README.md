# Attest

[![CI](https://github.com/mhomaid/attest/actions/workflows/ci.yml/badge.svg)](https://github.com/mhomaid/attest/actions/workflows/ci.yml)

SOC workbench for cloud alerts where every agent verdict is an Ed25519-signed
`AttestationEnvelope`. A classifier handles the cheap, repeatable cases (P99 under 5 ms
inference, asserted in CI); structurally novel ones escalate to an LLM through a policy-checked
MCP gateway. You can verify any envelope offline, and replay classifier decisions against the
recorded features and model hash.

```mermaid
flowchart LR
  CT[CloudTrail / Okta / M365] --> Col[attest-collector]
  Col --> RP[(Redpanda)]
  RP --> RW[RisingWave hot tier]
  RP --> PQ[Parquet on MinIO]
  RP --> Det[HELIQL detections]
  PQ --> CH[ClickHouse warm query]
  RW --> CP[control-plane]
  CH --> CP
  Det --> Orch[orchestrator]
  CP --> Orch
  Orch --> ONNX[ONNX classifier]
  Orch --> MCP[MCP gateway]
  Orch --> Log[(attestations.ndjson)]
  CP --> WB[workbench]
  Orch --> WB
```

## Status

Working = code + test. Partial = real code, incomplete vs the design docs. Planned = not built.

| Component | Status | Notes |
|---|---|---|
| CloudTrail → OCSF collector + Redpanda | Working | `POST /ingest`; shared types in `attest-common` |
| RisingWave hot tier + control-plane | Working | `recent_events`, `entity_baselines`; E2E Phase 1 |
| Warm storage | Partial | **Parquet on MinIO today.** The crate is named `attest-storage-iceberg` but there is no Iceberg catalog dependency in the write path. Iceberg is planned. |
| Warm query (`POST /v1/warm/query`) | Working | Tokenized SQL guard + ClickHouse `readonly=2`, 30 s / 10k-row limits. `s3()` pinned to the warm bucket. |
| HELIQL detections | Working | `attest-heliql` parses and compiles; 10 rules in `detections/` |
| Hybrid triager (classifier path) | Working | ONNX via `tract-onnx`; release P99 &lt; 5 ms asserted |
| Feature attribution | Partial | **SHAP-style**, not TreeSHAP: 8 extra model runs, one feature replaced by the background mean |
| LLM escalation + investigator | Working | Needs a configured LLM; live suites gated on `ATTEST_PHASE7_LIVE` |
| Shadow check + auto-close | Working | Real logic in `attest-shadow-check` (orchestrator re-exports it) |
| MCP gateway + tool policy | Working | Per-role authorize; `query_warm_tier` rate-limited |
| Signed attestation envelopes | Working | Ed25519 over SHA-256 of sorted-key canonical JSON |
| `attest verify` / `attest replay` | Working | Classifier path is deterministic; LLM path is integrity-only |
| Attestation log durability | Partial | Local NDJSON. No hash chain or external anchoring yet |
| Workbench (queue, case, hunt, simulate, load) | Working | Next.js 16, Better Auth, Playwright across 3 browsers |
| Hunter / responder / coordinator agents | Planned | Design only |
| AADF / SIDM / eval harness | Planned | Design docs in `docs/` |
| Helm / Terraform | Planned | — |

## Quick start

Needs Docker, Rust 1.98, Bun 1.4, uv (Python 3.12), cmake + libcurl + OpenSSL headers.

```sh
git clone https://github.com/mhomaid/attest.git
cd attest

# Infra + app services (Redpanda, RisingWave, ClickHouse, MinIO, Postgres, collector, control-plane, …)
make dev-up-all

# Auth schema + seeded analyst
cd infra/db && uv sync && DATABASE_URL=postgres://attest:attest@127.0.0.1:5432/attest uv run alembic upgrade head && cd ../..

# Workbench
cp apps/workbench/.env.local.example apps/workbench/.env.local
# set BETTER_AUTH_SECRET to any ≥32-char value for local dev
cd apps/workbench && bun install && bun run dev
```

Sign in at http://localhost:3000/login as `analyst@attest.local` / `analyst-dev`.

```sh
# Health
curl -s localhost:4000/healthz
curl -s localhost:8080/healthz

# Inject a CloudTrail login (then watch the queue)
curl -s -X POST localhost:4000/ingest -H 'Content-Type: application/json' \
  -d '{"Records":[{"eventName":"ConsoleLogin","eventTime":"'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'",
    "awsRegion":"ap-southeast-1","recipientAccountId":"111111111111",
    "userIdentity":{"type":"IAMUser","userName":"alice@example.com",
    "arn":"arn:aws:iam::111111111111:user/alice","accountId":"111111111111"}}]}'

# Or ~30s of mixed benign + attack traffic
make seed-data
```

Classifier artifacts (`ml/triager/artifacts/`) are committed, so you do not need to train to
triage. To retrain: `make train-classifier`.

## Verify and replay a decision

```sh
# After the orchestrator has written envelopes (ATTEST_LOG_PATH, default ./attestations.ndjson)
cargo run -q -p attest-cli -- verify ./attestations.ndjson --key "$ATTEST_VERIFYING_KEY"

cargo run -q -p attest-cli -- replay "$ACTION_ID" \
  --log ./attestations.ndjson \
  --key "$ATTEST_VERIFYING_KEY" \
  --model ml/triager/artifacts/model.onnx
```

`verify` recomputes the canonical hash and checks the Ed25519 signature (non-zero exit on any
failure). `replay` on a classifier envelope re-runs the ONNX model on the recorded features
and asserts `raw_prediction` within 1e-6 plus the stored verdict. On an LLM envelope it checks
the signature, that `system_prompt_hash` is present, and that tool-call timestamps are in
order — it does **not** regenerate the model output.

The orchestrator prints its verifying key at startup (`ATTEST_SIGNING_KEY` unset → ephemeral
key, fine for local, useless for audit).

## Tests

| Tier | Command | Docker? |
|---|---|---|
| Rust fmt / clippy / unit | `cargo fmt --all -- --check` · `make lint` · `cargo test --workspace` | no |
| Workbench unit + types | `bun run test` · `bun run typecheck` · `bun run lint` | no |
| ML | `cd ml && uv run pytest triager/ test_smoke.py -q` | no |
| Classifier latency | `cargo test --release -p attest-onnx-runtime --test classifier_integration` | no |
| Verify / replay | `cargo test -p attest-cli` | no |
| Workbench browsers | `cd apps/workbench && bun run e2e` (Postgres + migrations) | Postgres |
| Platform phases | `make e2e-phase1` … `make e2e-phase6` after `make dev-up-all` | yes |

Rust suites under `tests/e2e-tests` no-op unless `ATTEST_E2E=1` (the `make e2e-*` targets set it).
See `CONTRIBUTING.md` for the full matrix.

## Design docs

These describe the **target** system. Several talk about replay, Iceberg, and extra agents in
the present tense — the table above is the source of truth for today.

| Doc | |
|---|---|
| [docs/README.md](docs/README.md) | Index and reading order |
| [01 PRD](docs/01_PRD.md) | Requirements |
| [02 Architecture](docs/02_Architecture.md) | Six-plane model |
| [03 Diagrams](docs/03_Architecture_Diagrams.md) | C4 / sequences |
| [07 Stack](docs/07_Stack_Revised.md) | Canonical tech stack |
| [09 Agent harness](docs/09_Agent_Harness.md) | Execution paths and envelopes |
| [10 Build order](docs/10_Build_Order.md) | Phase sequence |
| [12 Workbench](docs/12_Workbench.md) | Analyst UX |
| [15 Streaming ADR](docs/15_Streaming_Engine_Decision.md) | Arroyo vs Flink vs RisingWave |
| [phases.md](docs/phases.md) | What actually shipped, phase by phase |

## Roadmap

- Hash-chain / anchor the attestation log so deleting an envelope is detectable
- Content-addressed store for tool args/results (replay investigator steps from recorded responses)
- Iceberg catalog on top of the existing Parquet layout
- TreeSHAP (or an honest rename everywhere if we keep the perturbation approximation)
- Eval harness (`eval/`) and the remaining agents (hunter, responder, coordinator)
- Pin service image tags; Helm/Terraform

## License and contributing

Apache-2.0. See [LICENSE](LICENSE), [CONTRIBUTING.md](CONTRIBUTING.md),
[SECURITY.md](SECURITY.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

Report vulnerabilities privately —
[GitHub advisories](https://github.com/mhomaid/attest/security/advisories/new) or mhomaid@gmail.com.
