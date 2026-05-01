# Phase E2E runners

Canonical entry points live in the **root `Makefile`** (`e2e-phase1` … `e2e-phase7`, `e2e-phase7-live`, `e2e-arroyo`).  
This directory adds a **single shell driver** for CI shells and muscle memory.

**`.env`:** `make run-workbench-api`, `run-orchestrator`, `run-mcp-gateway`, `run-control-plane`, and `make e2e-phase7-live` source the repo-root `.env` when it exists (Docker Compose already does this for containers). Put `ATTEST_LOG_PATH`, `ATTEST_LLM_API_KEY`, and optional Phase 7 toggles there instead of shell `export`.

```bash
./scripts/e2e/run-phase.sh 7              # Investigator integration (no Docker)
./scripts/e2e/run-phase.sh 7-live         # Live: orchestrator triage + workbench trace
./scripts/e2e/run-phase.sh 1              # Requires stack + ATTEST_E2E=1
./scripts/e2e/run-phase.sh all-platform   # Phases 1–3 serially (needs platform)
./scripts/e2e/run-phase.sh all-offline    # Everything that does not need Docker
```

## Prerequisites by phase

| Phase | Needs | Command |
|-------|--------|---------|
| **1–3** | `make dev-up-all` (or equivalent), `ATTEST_E2E=1` | `make e2e-phase1` … or `run-phase.sh 1` |
| **4a** | Starts calibration + orchestrator via Makefile | `make e2e-phase4a` |
| **4b** | Local LLM (Unsloth :8888), `ATTEST_LLM_PROVIDER=local` | `make e2e-phase4b` |
| **5** | Local LLM + guardrails | `make e2e-phase5` |
| **6** | Same style as 4a/5 | `make e2e-phase6` |
| **7** | **None** (WireMock + scripted LLM in Rust test) | `make e2e-phase7` |
| **7-live** | Running orchestrator + workbench-api, **same** `ATTEST_LOG_PATH`, LLM + MCP | `make e2e-phase7-live` |
| **Arroyo** | Platform profile | `make e2e-arroyo` |

## Phase 7 full stack (manual)

1. `make dev-up-all` (orchestrator, mcp-gateway, control-plane, calibration, tempo, …).
2. `make run-calibration` if orchestrator runs **outside** compose.
3. `make run-workbench-api` with `ATTEST_LOG_PATH` matching the orchestrator.
4. Workbench Next app: set `WORKBENCH_API_URL=http://localhost:4400`, `CONTROL_PLANE_URL`, `ORCHESTRATOR_URL`.
5. Open a case → **Attestation trace** panel; Hunt → **Run against warm tier** with `SELECT …`.

**Automated live proof:** `make e2e-phase7-live` runs `tests/e2e-tests/tests/phase7_live_investigator.rs` (real `POST /triage`, investigator when the model returns `needs_investigation`, then `GET /v1/cases/{id}/trace`). This is **not** the same as `make e2e-phase7`, which stays **offline**: it exercises `run_investigator_llm_loop` in-process with a fake chat client and WireMock MCP only.

For stricter CI, set `ATTEST_PHASE7_LIVE_STRICT=1`. To require a warm-tier MCP call: `PHASE7_REQUIRE_WARM_QUERY=1`.

See `docs/10_Build_Order.md` § Phase 7 for acceptance goals vs MVP implementation.
