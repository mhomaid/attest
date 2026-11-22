# Definition of done (every task)

- [ ] Every acceptance criterion maps to at least one test that passes, named in **Evidence**.
- [ ] Guards changed or added have negative tests.
- [ ] `verify.sh full` exits 0. Its summary line is pasted in **Evidence**.
- [ ] `check-org-leaks.sh` exits 0.
- [ ] No new `#[ignore]`, `.skip`, `xfail`, loosened assertion or raised budget.
- [ ] `cargo clippy -D warnings` clean; `bun run lint` and `bun run typecheck` clean if TS changed.
- [ ] New settings documented in `env.example` and `infra/helm/attest/values.yaml`.
- [ ] README Status table updated if a component changed status.
- [ ] ADR added if an architectural choice was made.
- [ ] Telemetry or third-party calls are opt-in and off by default.
- [ ] Commit is signed off (`-s`) and references the task ID.

## Extra checks by area

| If you touched | Also run |
| --- | --- |
| `crates/attest-attestation`, `attest-cli` | `cargo test -p attest-attestation -p attest-cli` and the offline example in `examples/verify` (3/3 pass, tamper fails) |
| `crates/attest-onnx-runtime`, `attest-feature-extractor`, `ml/` | release latency test: `cargo test --release -p attest-onnx-runtime --test classifier_integration`; `uv run pytest` in `ml/` |
| `crates/attest-heliql`, `detections/` | rule fixture tests (positive and negative per rule) |
| `crates/attest-mcp-gateway`, `attest-policy-engine` | policy tests including a denial for every new tool and role |
| `apps/workbench` | `bun run typecheck`, `bun run lint`, `bun run test`, Playwright for changed flows |
| `docker-compose.yml`, `infra/` | `docker compose up -d --wait` smoke, `make helm-template`, `make tf-validate` |
