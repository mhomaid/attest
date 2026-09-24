# Attest

[![CI](https://github.com/mhomaid/attest/actions/workflows/ci.yml/badge.svg)](https://github.com/mhomaid/attest/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Site](https://img.shields.io/badge/site-attest.homaid.dev-0f766e)](https://attest.homaid.dev)

**AI SOCs ask you to trust the verdict. Attest makes it something you can check.**

Every agent decision is an Ed25519-signed `AttestationEnvelope` on a hash chain. A cheap
ONNX classifier handles the repeatable cases (P99 under 5 ms, asserted in CI). Structurally
novel ones escalate to an LLM through a policy-checked MCP gateway. `attest verify` fails if
anyone tampers with a row, swaps a model hash, or breaks the chain.

Built by [Mohamed Homaid](https://github.com/mhomaid) · [attest.homaid.dev](https://attest.homaid.dev)

**Status: early open-source prototype.** Single tenant, not a production service, no customers.
The [status table](#status) is the source of truth. The hosted site is a waitlist and
a paused demo — the [offline verify example](#verify-and-replay-a-decision) is the path that
always works. It needs only Rust.

## Why this exists

When an agent closes an alert or isolates a host, most tools leave a chat transcript. An
auditor cannot replay it. A swapped model or a deleted row leaves no mark. A CISO still has
to defend the action. Trusting the vendor's UI is not an audit trail.

| | |
|---|---|
| **Problem** | You cannot prove the verdict is the one the agent produced, or that the model was the one you approved. |
| **Why we built it** | Someone still has to stand behind the action. The log has to survive a skeptical reviewer. |
| **How** | Sign the envelope, chain it to the row before it, verify offline. Same CLI we run in CI. |

The slice that is real today is narrow on purpose:

**CloudTrail in → signed verdict out → verify next week.**

That is the claim. Not “AI SOC,” not a hosted multi-tenant product.

## How a verdict becomes checkable

```mermaid
sequenceDiagram
  autonumber
  participant CT as CloudTrail
  participant Col as collector
  participant Bus as Redpanda
  participant Det as HELIQL / RisingWave
  participant Orch as orchestrator
  participant ONNX as ONNX classifier
  participant MCP as MCP gateway
  participant Log as attestation log
  participant CLI as attest verify

  CT->>Col: JSON (POST /ingest or S3)
  Col->>Bus: OCSF 1.3 event
  Bus->>Det: stream
  Det->>Orch: fired alert
  Orch->>ONNX: 8 features, in-process
  alt structurally novel
    Orch->>MCP: investigator tools (policy-checked)
  end
  Orch->>Log: Ed25519 envelope + prev_hash
  CLI->>Log: recompute hash, check signature, walk chain
```

What the envelope commits to:

- Canonical JSON (sorted keys) → SHA-256 → Ed25519
- `prev_hash` of the previous envelope (deleting or reordering a middle row fails verify)
- Classifier path: features, model hash, `raw_prediction` — `attest replay` re-runs ONNX
- LLM path: `system_prompt_hash` and tool-call order — integrity only, not regenerated output

Limits we do not hide: the chain does not yet prove tail truncation, and a holder of
`ATTEST_SIGNING_KEY` can re-sign a new chain. External tip anchoring is [on the roadmap](#roadmap).

## Architecture

Six planes. Agents never open a database or a credential; the MCP gateway is the only door.

```mermaid
flowchart TB
  subgraph Workbench["01 Workbench — where analysts work"]
    WB[Next.js 16 workbench]
    CP[Rust control-plane]
  end
  subgraph Agentic["02 Agentic — decide and sign"]
    Orch[orchestrator]
    ONNX[tract-onnx classifier]
    MCP[MCP gateway + policy]
    Att[hash-chained log]
  end
  subgraph Detection["03 Detection — write a rule once"]
    HQ[HELIQL → RisingWave / ClickHouse]
  end
  subgraph Storage["04 Storage — open formats you own"]
    IB[Iceberg / Parquet]
    CH[ClickHouse s3]
  end
  subgraph Streaming["05 Streaming — logs in, OCSF on a bus"]
    Col[Rust collector]
    RP[Redpanda]
    RW[RisingWave]
  end
  subgraph Control["06 Control — identity and Git"]
    Auth[Better Auth + Postgres]
    IaC[Compose · Helm · Terraform]
  end

  Col --> RP --> RW
  RP --> IB
  RW --> CP
  IB --> CH --> CP
  HQ --> Orch
  CP --> Orch
  Orch --> ONNX
  Orch --> MCP
  Orch --> Att
  CP --> WB
  Orch --> WB
```

```mermaid
flowchart LR
  CT[CloudTrail] --> Col[attest-collector]
  Col --> RP[(Redpanda)]
  RP --> RW[RisingWave hot tier]
  RP --> PQ[Iceberg on object storage]
  RP --> Det[HELIQL detections]
  PQ --> CH[ClickHouse warm query]
  RW --> CP[control-plane]
  CH --> CP
  Det --> Orch[orchestrator]
  CP --> Orch
  Orch --> ONNX[ONNX classifier]
  Orch --> MCP[MCP gateway]
  Orch --> Log[(NDJSON + optional S3)]
  CP --> WB[workbench]
  Orch --> WB
```

Object storage is **S3-compatible**. Local Compose uses a pinned MinIO rebuild; a hosted
deployment should point at S3 or R2. The application talks the S3 API, not a vendor SDK.

C4 diagrams, sequences, and the full six-plane write-up live in
[docs/02_Architecture.md](docs/02_Architecture.md) and
[docs/03_Architecture_Diagrams.md](docs/03_Architecture_Diagrams.md).

## Tech stack

Chosen so a small team can run the same path on a laptop and in a VPC. Rust on every hot
path. No JVM in the data plane. Open table formats so you can leave with the data.

| Layer | Choice | Why |
|---|---|---|
| Collect | Rust · axum · OCSF 1.3 | One binary; CloudTrail JSON in, common schema out |
| Bus | Redpanda (Kafka API) | Single binary, no ZooKeeper |
| Hot stream | RisingWave | Streaming SQL, Postgres protocol, materialized baselines |
| CEP / ETL | Arroyo | Multi-step sequences without Flink |
| Warm store | Apache Iceberg + Parquet | Same files locally (`file://` / MinIO) and on S3 |
| Hunt | ClickHouse `s3()` | Query the warm files; no second copy |
| Detect | HELIQL (Rust · pest) + Sigma import | One rule, stream or hunt |
| Triage | XGBoost → ONNX via `tract-onnx` | P99 &lt; 5 ms in CI; no Python on the hot path |
| Calibrate | Python · uv sidecar | Isotonic calibration and novelty; training only |
| Agents | Rust orchestrator · MCP gateway | Policy, budgets, signed envelopes |
| Sign / verify | Ed25519 · SHA-256 · `attest` CLI | Offline check; classifier replay |
| UI | Next.js 16 · Bun · Zustand · Better Auth | Marketing site + SOC workbench |
| Local | Docker Compose (`attest`) | `make dev-up-all` |
| Ship | GitHub Actions · Helm · Terraform (AWS / GCP / Azure) | Store modules + chart. Does not create the cluster. |

Decision record: [docs/07_Stack_Revised.md](docs/07_Stack_Revised.md) and
[docs/15_Streaming_Engine_Decision.md](docs/15_Streaming_Engine_Decision.md) (Arroyo vs Flink
vs RisingWave).

## Status

Working = code + test. Partial = real code, incomplete vs the design docs. Planned = not built.

| Component | Status | Notes |
|---|---|---|
| CloudTrail → OCSF collector + Redpanda | Working | `POST /ingest` (tenant from `X-Tenant-Id`, else `TENANT_ID`). `POST /ingest/s3` or an S3 object-created notification fetches a gzipped CloudTrail file; buckets must be listed in `COLLECTOR_S3_ALLOWED_BUCKETS`. No ingest auth yet: keep the collector on a private network. |
| RisingWave hot tier + control-plane | Working | `recent_events`, `entity_baselines`; E2E Phase 1 |
| Warm storage | Working | Iceberg table `attest.cloudtrail`: Parquet data files + snapshot commits via iceberg-rust. Catalog pointer is `metadata/version-hint.text` so a restarted writer reloads the same table. Warehouse is `s3://attest-warm` (MinIO) or a local `file://` dir. |
| Warm query (`POST /v1/warm/query`) | Working | Tokenized SQL guard + ClickHouse `readonly=2`, 30 s / 10k-row limits. `s3()` pinned to the warm bucket. |
| HELIQL detections | Working | `attest-heliql` parses and compiles; 10 rules in `detections/` |
| Hybrid triager (classifier path) | Working | ONNX via `tract-onnx`; release P99 &lt; 5 ms asserted |
| Feature attribution | Partial | **SHAP-style**, not TreeSHAP: 8 extra model runs, one feature replaced by the background mean |
| LLM escalation + investigator | Working | Needs a configured LLM; live suites gated on `ATTEST_PHASE7_LIVE` |
| Shadow check + auto-close | Working | Real logic in `attest-shadow-check` (orchestrator re-exports it) |
| MCP gateway + tool policy | Working | Per-role authorize; warm-query exfil cap; poisoned tool results denied |
| Signed attestation envelopes | Working | Ed25519 over SHA-256 of sorted-key canonical JSON |
| `attest verify` / `attest replay` | Working | Classifier path is deterministic; LLM path is integrity-only. Verify walks the hash chain and rejects duplicate `agent_action_id`. |
| Attestation log durability | Partial | Hash-chained NDJSON. With `ATTEST_LOG_S3_BUCKET` set, every envelope is also written to S3/MinIO first, and a fresh disk rebuilds from it. Orchestrator serves `GET /v1/attestations`, `/export`, `/verify`, `/{id}`. No external anchoring of the chain tip yet. |
| Workbench (queue, case, hunt, simulate, load) | Working | Next.js 16, Better Auth, Playwright across 3 browsers |
| Hunter / responder / coordinator agents | Partial | Coordinator is deterministic routing (`POST /v1/coordinate`), not an LLM planner. Hunter (`POST /v1/hunt`) and Responder (`POST /v1/respond`) are LLM paths with a signed-envelope fallback when no model is configured; not yet exercised against a live model in CI. Responder actions are shadow-checked and only *recorded as planned*: nothing is executed against a real IdP/EDR. |
| AADF / SIDM / eval harness | Planned | Design docs in `docs/` |
| Helm / Terraform | Working | Reference chart in `infra/helm/attest`. AWS BYOC module in `infra/terraform/aws` (warm S3 bucket + optional Helm release onto an existing EKS cluster). |

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

Sign in at http://localhost:3000/login as `analyst@attest.local` / `analyst-dev`
(or the public demo user `demo@attest.local` / `try-attest`). The homepage **Run the demo**
button runs ingest → triage → verify for tenant `demo` against your local collector and
orchestrator. If either is down, the section says the demo is paused and points to the
verify example instead.

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

The binary is `attest` (crate `attest-cli`). Full flags and exit codes:
[crates/attest-cli/README.md](crates/attest-cli/README.md).

Install once, then call `attest` (the `--` after `cargo run` is required if you skip install):

```sh
cargo install --path crates/attest-cli --locked
# or: cargo run -q -p attest-cli -- <subcommand> …
```

No stack needed: `examples/verify/` holds three chained envelopes signed with a public demo key
(regenerate with `cargo run -q -p attest-cli --example make_sample`). Prefer `--key-file` so the
hex is not in `ps` or shell history.

```sh
attest verify examples/verify/attestations.ndjson \
  --key-file examples/verify/verifying-key.txt          # verified 3/3

# Copy, then delete a row — do not sed -i the file in git
sed '2d' examples/verify/attestations.ndjson > /tmp/broken.ndjson
attest verify /tmp/broken.ndjson --key-file examples/verify/verifying-key.txt

attest verify examples/verify/attestations.ndjson \
  --key-file examples/verify/verifying-key.txt --pin-model 0000   # model swap
```

```sh
# After the orchestrator has written envelopes (ATTEST_LOG_PATH, default ./attestations.ndjson)
attest verify ./attestations.ndjson --key-file ./verifying-key.txt
# or: export ATTEST_VERIFYING_KEY=… and omit --key / --key-file

attest replay "$ACTION_ID" \
  --log ./attestations.ndjson \
  --key-file ./verifying-key.txt \
  --model ml/triager/artifacts/model.onnx
```

`verify` recomputes the canonical hash and checks the Ed25519 signature (exit 1 on any
failure). `replay` on a classifier envelope re-runs the ONNX model on the recorded features
and asserts `raw_prediction` within 1e-6 plus the stored verdict. On an LLM envelope it checks
the signature, that `system_prompt_hash` is present, and that tool-call timestamps are in
order — it does **not** regenerate the model output.

The orchestrator prints its verifying key at startup (`ATTEST_SIGNING_KEY` unset → ephemeral
key, fine for local, useless for audit: after a restart, earlier envelopes no longer verify
against the new key). Any long-lived deployment must set it.

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
the present tense — the [status table](#status) is the source of truth for today.

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
| [attest CLI](crates/attest-cli/README.md) | `verify` / `replay` flags and exit codes |

## Roadmap

Attest is a v0 lab: the ingest → sign → verify loop works. The roadmap is ordered by
**what would fail an audit first**, then by **what would fail a self-host**, then by
**product surface**. No dates — work lands when it has a test.

```mermaid
flowchart LR
  V0["v0 · today<br/>CloudTrail → signed verdict → verify"]
  A["1 · Audit-grade log"]
  B["2 · Operable self-host"]
  C["3 · Broader SOC surface"]
  D["Later · research tracks"]
  V0 --> A --> B --> C
  B -.-> D
```

### 1 — Audit-grade log

Close the integrity gaps a skeptical reviewer will find first.

| Item | Outcome | Today |
|---|---|---|
| External tip anchor | Publish the chain tip to a transparency log or object-lock bucket so dropping the last *n* rows is detectable | Hash chain only; tail truncation is still valid |
| Stable signing key as default for any durable deploy | Envelopes still verify after a restart | Unset `ATTEST_SIGNING_KEY` → ephemeral key |
| Content-addressed tool I/O | Replay investigator steps from recorded args/results, not from a live model | LLM path is integrity-only |
| Honest attribution | TreeSHAP, or rename the current 8-run perturbation everywhere we say SHAP | SHAP-style, not TreeSHAP |

### 2 — Operable self-host

Make a clone something you would leave running, not just demo.

| Item | Outcome | Today |
|---|---|---|
| Authenticated ingest | Per-tenant tokens on the collector | Private-network only; no ingest auth |
| Iceberg REST catalog | Restart-safe catalog that is not `MemoryCatalog` + `version-hint.text` | Snapshot reload works; catalog is local |
| Pinned service images | Compose / Helm tags that do not float | Compose infra images are pinned. App images are still built from source. |
| S3-native object store path | Documented Garage / S3 / R2 swap; RisingWave `hummock+s3://` | Local default is a pinned MinIO rebuild |
| Helm + Terraform beyond the reference | Same chart onto an existing EKS cluster with fewer manual steps | Store modules for AWS, GCP, and Azure. Helm is `k8s-release` / `eks-release`. None of them create the cluster. |

### 3 — Broader SOC surface

Only after the log and the self-host path are something we can defend.

| Item | Outcome | Today |
|---|---|---|
| More sources | Okta, Entra, EDR on the same OCSF collector | CloudTrail is live |
| Live Hunter / Responder | Hunt and planned-containment exercised against a real model in CI | Endpoints exist; fallback is a signed envelope without an LLM |
| Responder connectors | Recorded actions can be executed against a real IdP / EDR, still shadow-checked | Planned only; nothing is sent to a vendor API |
| Eval harness (`eval/`) | Golden cases gate classifier + investigator quality in CI | Directory exists; harness is not built |
| AADF / SIDM | Agent-aware detections and self-improving rules, as designed in `docs/` | Design only |

### Explicitly not on this roadmap

These show up in the design docs as year-two or GA language. They are **not** promised here:

- Hosted multi-tenant SaaS, billed BYOC, or an air-gapped appliance
- SOC 2 / ISO 27001 / FedRAMP / HIPAA eligibility
- Replacing a SIEM, or executing containment in a customer environment

If a line is not in the [status table](#status), it is not shipped.

## License and contributing

Apache-2.0. See [LICENSE](LICENSE), [CONTRIBUTING.md](CONTRIBUTING.md),
[SECURITY.md](SECURITY.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

Report vulnerabilities privately to **Mohamed Homaid** via
[GitHub advisories](https://github.com/mhomaid/attest/security/advisories/new)
or mhomaid@gmail.com.
