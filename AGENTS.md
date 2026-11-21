# AGENTS.md — working on Attest

Attest is an open-source (Apache-2.0) AI SOC layer: alerts in, OCSF on a bus, a fast ONNX
triager, an LLM investigator behind a policy-checked MCP gateway, and every decision written
as an Ed25519-signed, hash-chained `AttestationEnvelope` that `attest verify` can check offline.

This file is for coding agents (Cursor, Claude Code, Codex). Humans should read `README.md`
and `CONTRIBUTING.md` first.

## Where the work is defined

| File | What it is |
| --- | --- |
| `TASKS.md` | The task board. Every unit of work has an ID, status, dependencies, acceptance criteria and an evidence log. **Work only from this file.** |
| `docs/16_Production_Readiness_Plan.md` | The plan the tasks come from: why each milestone exists and how the pieces fit. |
| `docs/02_Architecture.md`, `docs/09_Agent_Harness.md`, `docs/10_Build_Order.md` | Existing design docs. The build-order rule ("tests gate progress") still applies. |
| README `Status` table | The public source of truth for what works. Update it when a task changes a component's status. |

## Skills (in `.cursor/skills/`)

| Skill | Use it when |
| --- | --- |
| `attest-work-loop` | Doing any task from `TASKS.md`. It runs the plan → test → implement → verify → record loop and does not stop until the task is proven done or truly blocked. |
| `attest-plan-intake` | Someone gives you a plan, doc, issue list or meeting notes to turn into tasks. |
| `attest-add-source` | Adding a new log or alert source to the collector. |
| `attest-add-mcp-tool` | Adding or replacing a tool in the MCP gateway. |
| `attest-add-detection` | Writing a HELIQL rule or importing Sigma. |
| `attest-verify` | Checking work. Run it before you say anything is done. |
| `attest-upstream-hygiene` | Before every commit. Keeps organization-specific content out of this public repo. |

## Commands

```bash
make fmt                     # cargo fmt
make lint                    # clippy -D warnings + bun lint
make test                    # cargo test + bun test + pytest
make smoke                   # quick cross-language sanity check
make e2e-all-offline         # Rust workspace + investigator test + ML pytest, no Docker
make dev-up-all              # full local stack in Docker
.cursor/skills/attest-verify/scripts/verify.sh quick|full     # the gate used by the work loop
.cursor/skills/attest-upstream-hygiene/scripts/check-org-leaks.sh   # org content check
make check-hygiene           # leak-check fixtures, DCO fixtures, then the leak check
```

## Hard rules

1. **This repository is public and organization-neutral.** No company names, domains,
   hostnames, account IDs, stream names, incident IDs, customer data or real log samples.
   Organization-specific configuration lives in a private overlay repo, never here.
   Run the hygiene check before every commit.
2. **Tests gate progress.** A task is done only when its acceptance criteria pass, `verify.sh`
   is green, and the evidence is written in `TASKS.md`. "Should work" is not done.
3. **Do not weaken the trust core** (`attest-attestation`, `attest-policy-engine`,
   `attest-mcp-gateway` authorization) to make a test pass. If a change touches them, add
   negative tests that prove the guard still refuses what it must refuse.
4. **Agents never get direct storage or credential access.** New data access goes through
   an MCP tool with a policy entry and caps.
5. **Synthetic fixtures only.** Build test events from vendor documentation and generators,
   never from production logs, even redacted ones.
6. **No telemetry by default.** Anything that sends data off the host is opt-in and off
   unless an env var turns it on.
7. **Small, reviewable commits.** One task per branch/PR where possible; conventional commit
   messages; `Signed-off-by` (DCO) on every commit (`git commit -s`).
8. **Ask, don't guess, on product decisions.** If acceptance criteria are ambiguous, record
   the question under the task in `TASKS.md`, mark it `blocked: decision`, and move on.

## Layout

```
crates/attest-collector        ingest + OCSF normalization (sources live here)
crates/attest-common           OCSF types, FlatEvent
crates/attest-feature-extractor  classifier features
crates/attest-onnx-runtime     tract-onnx inference, P99 budget test
crates/attest-orchestrator     triage, LLM loop, coordinator, specialists
crates/attest-mcp-gateway      tool registry + dispatch (tools.rs)
crates/attest-policy-engine    per-role authorization
crates/attest-attestation      envelope, signer, log, verify, pin
crates/attest-heliql           grammar.pest, parser, compiler, sigma import
crates/attest-cli              `attest verify`, `attest replay`
apps/workbench                 Next.js 16 analyst UI (Better Auth)
detections/                    HELIQL rules
ml/triager                     training + calibration (Python, uv)
infra/helm, infra/terraform    deployment
```
