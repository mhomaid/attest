# Contributing to Attest

Thanks for taking a look. Attest is a solo-maintained, pre-1.0 project; testers, issues, and
focused PRs are welcome. For anything larger than a bug fix, open an issue first so we can
agree on scope.

## Try it without cloning

The hosted site (`https://attest.homaid.dev`) has a public demo on the homepage: **Try a live
verdict**. That posts a CloudTrail `ConsoleLogin` for tenant `demo`, runs triage, and walks
the hash chain. If collector or orchestrator is down, the page reports the failure instead of
faking a pass.

Workbench login on a seeded database:

| Email | Password | Role |
| ----- | -------- | ---- |
| `demo@attest.local` | `try-attest` | public demo |
| `analyst@attest.local` | `analyst-dev` | local analyst |

Do not use these passwords outside local / hosted demo.

## Prerequisites

| Tool | Version | Used for |
| ---- | ------- | -------- |
| Rust | 1.98.1 (`rust-toolchain.toml`; images use `rust:1.98-slim`) | all services under `crates/`, `apps/*` Rust binaries |
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
| Verify / replay CLI | `cargo test -p attest-cli` | no |
| Org leak + DCO | `make check-hygiene` | no |
| Platform E2E, per phase | `make dev-up-all`, then `make e2e-phase1` … `make e2e-phase6` | yes |

Offline attestation (after the orchestrator has written `attestations.ndjson`).
See [crates/attest-cli/README.md](crates/attest-cli/README.md). Prefer `--key-file`.

```sh
cargo install --path crates/attest-cli --locked   # once
attest verify ./attestations.ndjson --key-file ./verifying-key.txt
attest replay "$ACTION_ID" --log ./attestations.ndjson \
  --key-file ./verifying-key.txt --model ml/triager/artifacts/model.onnx

# checkout without installing — the `--` is required
cargo run -q -p attest-cli -- verify ./attestations.ndjson --key-file ./verifying-key.txt
```

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
5. Sign off every commit (Developer Certificate of Origin): `git commit -s`.
   Pull requests fail the DCO check otherwise. Run `scripts/tests/dco_check_test.sh`
   to see the check accept a signed commit and reject an unsigned one.
6. Architectural changes (new service, new storage tier, a change to the attestation envelope)
   need an ADR in `docs/` — see `docs/15_Streaming_Engine_Decision.md` for the format.

### Keep organization-specific content out

This repository is public. Company names, domains, hostnames, account IDs, stream names,
incident IDs, and real log lines belong in a private overlay, not here. Before you commit:

```sh
make check-hygiene
```

That runs three things:

| Command | What a pass means |
| --- | --- |
| `scripts/tests/leak_check_test.sh` | A planted deny-list term, a non-placeholder AWS account id, and a private-key header each fail the check and the output is `file:line` only. The AWS docs account `123456789012` passes. |
| `scripts/tests/dco_check_test.sh` | An unsigned commit fails `scripts/check-dco.sh`; a `Signed-off-by` commit passes. |
| `.cursor/skills/attest-upstream-hygiene/scripts/check-org-leaks.sh` | The current tree has no generic leak (real-looking account id or private key). |

The same leak check runs on pull requests (`.github/workflows/org-leak-check.yml`). It also
reads a private term list. Locally that list is `.attest-denylist` (gitignored), one extended
regex per line, `#` starts a comment:

```text
# .attest-denylist
acme\.example
```

In GitHub Actions the same text is the `ATTEST_ORG_DENYLIST` secret. Fork pull requests do not
receive the secret; they run the generic checks only. A hit prints `file:line` and never the
matched text. Remove the content. Do not add an allow list.

Optional local gate, from the repo root:

```sh
ln -sf ../../.cursor/skills/attest-upstream-hygiene/scripts/check-org-leaks.sh .git/hooks/pre-commit
chmod +x .cursor/skills/attest-upstream-hygiene/scripts/check-org-leaks.sh
```

Agent workflow for this repo lives in `AGENTS.md` and `TASKS.md`. The gate an agent must pass
before calling a task done is:

```sh
.cursor/skills/attest-verify/scripts/verify.sh quick
.cursor/skills/attest-verify/scripts/verify.sh full
```

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
