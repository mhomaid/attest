# Contributing to Attest

Thanks for taking a look. Attest is a solo-maintained, pre-1.0 project; issues and focused PRs
are welcome. For anything larger than a bug fix, open an issue first so we can agree on scope.

## Prerequisites

| Tool | Version | Used for |
| ---- | ------- | -------- |
| Rust | stable (CI uses latest stable; images use 1.95) | all services under `crates/`, `apps/*` Rust binaries |
| cmake, libcurl, OpenSSL headers | any recent | building `librdkafka` (`rdkafka` `cmake-build` feature) |
| Bun | 1.4.x | workbench (`apps/workbench`) |
| uv + Python | 3.12 | ML pipeline (`ml/`) and DB migrations (`infra/db`) |
| Docker + Compose v2 | recent | local stack (Redpanda, RisingWave, ClickHouse, MinIO, Postgres) |

On macOS: `brew install cmake openssl pkg-config`. On Debian/Ubuntu:
`apt-get install cmake libcurl4-openssl-dev libssl-dev pkg-config`.

## Test tiers

Everything CI runs, you can run locally. From the repo root:

| Tier | Command | Needs Docker |
| ---- | ------- | ------------ |
| Format | `cargo fmt --all -- --check` | no |
| Lint | `make lint` (clippy `-D warnings` + ESLint) | no |
| Unit + integration | `make test` (Rust workspace, workbench `bun test`, ML pytest) | no |
| Workbench types | `bun run typecheck` | no |
| Classifier latency budget | `cargo test --release -p attest-onnx-runtime --test classifier_integration` | no |
| Workbench browser E2E | `cd apps/workbench && bun run e2e` (needs Postgres with migrations applied) | Postgres only |
| Platform E2E, per phase | `make dev-up-all`, then `make e2e-phase1` … `make e2e-phase6` | yes |

The Rust suites under `tests/e2e-tests` skip themselves unless `ATTEST_E2E=1` is set, so
`cargo test --workspace` stays green without a running stack. The `make e2e-*` targets set it
for you.

Database migrations: `cd infra/db && uv run alembic upgrade head`
(`DATABASE_URL` defaults to the compose Postgres).

## Making a change

1. Branch from `main`.
2. Keep the change focused; add or update a test that fails without it.
3. Run the tiers above that cover what you touched. PRs must pass CI.
4. Use [Conventional Commits](https://www.conventionalcommits.org/) for messages
   (`feat(orchestrator): …`, `fix(workbench): …`, `docs: …`, `ci: …`).
5. Architectural changes (new service, new storage tier, a change to the attestation envelope)
   need an ADR in `docs/` — see `docs/15_Streaming_Engine_Decision.md` for the format.

### Changing the attestation envelope

`AttestationEnvelope` is a signed, persisted format. Adding a field changes the canonical bytes,
so old envelopes will no longer verify under new code unless the field is optional and omitted
when empty (`#[serde(default, skip_serializing_if = ...)]`). Bump `envelope_version` for any
change that is not backwards compatible, and add a verification test.

## Where things live

- `crates/` — Rust libraries and services (orchestrator, inference router, HELIQL compiler,
  attestation, MCP gateway, …).
- `apps/` — deployable binaries and the Next.js workbench.
- `ml/` — classifier training, novelty detection, calibration sidecar.
- `detections/` — HELIQL detection rules.
- `docs/` — design documents. They describe the **target** architecture; the Status table in
  the root README is the source of truth for what is implemented.

## Reporting security issues

See [SECURITY.md](SECURITY.md). Please don't open public issues for vulnerabilities.

## Code of conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).
