---
name: attest-verify
description: Verify Attest changes with the same checks CI runs (fmt, clippy, tests, P99 latency budget, offline attestation verify, workbench typecheck/lint/test, ml pytest, helm/terraform, org-leak check). Use before claiming any task is done, after any refactor, or when asked "is it working" or "run the tests".
---

# Verify

Run the gate script from the repo root:

```bash
.cursor/skills/attest-verify/scripts/verify.sh quick   # scoped to changed areas
.cursor/skills/attest-verify/scripts/verify.sh full    # everything; required before "done"
```

The script works out which areas changed (Rust crates, workbench, ml, infra, attestation,
HELIQL) and runs the matching checks. In `full` mode a missing toolchain counts as a failure.
The last line is a one-line summary (`VERIFY PASS ...` or `VERIFY FAIL ...`). Paste it into the
task's **Evidence** in `TASKS.md`.

## Rules

- Exit code is the truth. Do not summarize a failing run as passing.
- When a check fails, fix the cause. Do not skip tests, add `#[ignore]`, loosen assertions, or
  raise the classifier latency budget.
- If a check cannot run locally (needs Docker, a cloud account, or a paid service), say which one
  in **Evidence** and rely on CI for it. Name the CI job.
- After the script passes, also run any acceptance check the task names explicitly (for example
  a specific e2e phase: `make e2e-phase4a`).

## What it runs

| Area changed | Checks |
| --- | --- |
| always | org-leak check |
| Rust | `cargo fmt --check`, `clippy -D warnings`, `cargo test` (changed crates in quick, workspace in full) |
| onnx runtime, features, ml | release P99 latency test (needs `ml/triager/artifacts/model.onnx`) |
| attestation, cli | crate tests + `examples/verify/run.sh` if present |
| HELIQL, detections | `cargo test -p attest-heliql` |
| workbench | `bun run typecheck`, `lint`, `test` |
| ml | `uv run pytest` |
| infra | `make helm-template`, `make tf-validate`, `docker compose config` |
