# Phase E2E runners

Canonical entry points live in the **root `Makefile`** (`e2e-phase1` … `e2e-phase7`, `e2e-arroyo`).  
This directory adds a **single shell driver** for CI shells and muscle memory:

```bash
./scripts/e2e/run-phase.sh 7              # Investigator integration (no Docker)
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
| **Arroyo** | Platform profile | `make e2e-arroyo` |

## Phase 7 full stack (manual)

1. `make dev-up-all` (orchestrator, mcp-gateway, control-plane, calibration, tempo, …).
2. `make run-calibration` if orchestrator runs **outside** compose.
3. `make run-workbench-api` with `ATTEST_LOG_PATH` matching the orchestrator.
4. Workbench Next app: set `WORKBENCH_API_URL=http://localhost:4400`, `CONTROL_PLANE_URL`, `ORCHESTRATOR_URL`.
5. Open a case → **Attestation trace** panel; Hunt → **Run against warm tier** with `SELECT …`.

See `docs/10_Build_Order.md` § Phase 7 for acceptance goals vs MVP implementation.
