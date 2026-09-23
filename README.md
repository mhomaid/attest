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
  RP --> PQ[Iceberg on MinIO]
  RP --> Det[HELIQL detections]
  PQ --> CH[ClickHouse warm query]
  RW --> CP[control-plane]
  CH --> CP
  Det --> Orch[orchestrator]
  CP --> Orch
  Orch --> ONNX[ONNX classifier]
  Orch --> MCP[MCP gateway]
  Orch --> Log[(hash-chained attestation log<br/>NDJSON + optional S3)]
  CP --> WB[workbench]
  Orch --> WB
```

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
| Hunter / responder / coordinator agents | Working | Coordinator is deterministic routing (`POST /v1/coordinate`). Hunter (`POST /v1/hunt`) and Responder (`POST /v1/respond`) are LLM paths with a signed-envelope fallback when no model is configured. Responder actions are shadow-checked and only *planned* against a real IdP/EDR. |
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
(or the public demo user `demo@attest.local` / `try-attest`). The homepage **Try a live
verdict** button runs ingest → triage → verify for tenant `demo` against your local collector
and orchestrator.

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
| [attest CLI](crates/attest-cli/README.md) | `verify` / `replay` flags and exit codes |

## Roadmap

- Anchor the attestation chain tip externally (transparency log / object lock)
- Authenticated, per-tenant ingest tokens on the collector
- Content-addressed store for tool args/results (replay investigator steps from recorded responses)
- TreeSHAP (or an honest rename everywhere if we keep the perturbation approximation)
- Eval harness (`eval/`)
- Pin service image tags; REST Iceberg catalog (today is MemoryCatalog + version-hint on the warehouse)
- Live IdP/EDR connectors behind the Responder (today the tools record a planned action)

## License and contributing

Apache-2.0. See [LICENSE](LICENSE), [CONTRIBUTING.md](CONTRIBUTING.md),
[SECURITY.md](SECURITY.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

Report vulnerabilities privately —
[GitHub advisories](https://github.com/mhomaid/attest/security/advisories/new) or mhomaid@gmail.com.
