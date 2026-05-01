#!/usr/bin/env bash
# Run Attest phase E2E tests. See scripts/e2e/README.md
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

PHASE="${1:-}"
shift || true

case "$PHASE" in
  1)
    exec env ATTEST_E2E=1 cargo test -p e2e-tests --test phase1_streaming -- "$@"
    ;;
  2)
    exec env ATTEST_E2E=1 cargo test -p e2e-tests --test phase2_iceberg -- "$@"
    ;;
  3)
    exec env ATTEST_E2E=1 cargo test -p e2e-tests --test phase3_detection -- "$@"
    ;;
  4a)
    exec make e2e-phase4a
    ;;
  4b)
    exec make e2e-phase4b
    ;;
  5)
    exec make e2e-phase5
    ;;
  6)
    exec make e2e-phase6
    ;;
  7)
    exec cargo test -p attest-orchestrator --test investigator_loop -- "$@"
    ;;
  7-live|7live)
    set -a
    [ -f "$ROOT/.env" ] && . "$ROOT/.env"
    set +a
    exec env ATTEST_E2E=1 ATTEST_PHASE7_LIVE=1 \
      cargo test -p e2e-tests --test phase7_live_investigator -- "$@"
    ;;
  arroyo)
    exec make e2e-arroyo
    ;;
  all-platform)
    echo "==> Phase 1 (requires ATTEST_E2E=1 + platform)..."
    ATTEST_E2E=1 cargo test -p e2e-tests --test phase1_streaming -- --nocapture --test-threads=1 "$@"
    echo "==> Phase 2..."
    ATTEST_E2E=1 cargo test -p e2e-tests --test phase2_iceberg -- --nocapture --test-threads=1 "$@"
    echo "==> Phase 3..."
    ATTEST_E2E=1 cargo test -p e2e-tests --test phase3_detection -- --nocapture --test-threads=1 "$@"
    echo "==> all-platform done."
    ;;
  all-offline)
    echo "==> Rust workspace tests..."
    cargo test --workspace -q
    echo "==> Phase 7 investigator (WireMock)..."
    cargo test -p attest-orchestrator --test investigator_loop -q
    echo "==> ML pytest..."
    (cd ml && env -u VIRTUAL_ENV uv run pytest triager/ test_smoke.py -q)
    echo "==> all-offline done."
    ;;
  ""|help|-h|--help)
    cat <<'EOF'
Usage: scripts/e2e/run-phase.sh <phase> [extra cargo args]

  1 | 2 | 3       Platform E2E (set ATTEST_E2E=1 via script for 1–3)
  4a | 4b | 5 | 6 Makefile targets (start/stop services as defined there)
  7               Investigator loop test (no Docker; WireMock + scripted LLM)
  7-live          Live HTTP: POST /triage → investigator → GET …/trace (see README)
  arroyo          make e2e-arroyo
  all-platform    Phases 1, 2, 3 serially
  all-offline     cargo test workspace + phase7 + ml pytest

Examples:
  ./scripts/e2e/run-phase.sh 7 -- --nocapture
  ./scripts/e2e/run-phase.sh 7-live -- --nocapture
  ATTEST_E2E=1 ./scripts/e2e/run-phase.sh all-platform
EOF
    exit 0
    ;;
  *)
    echo "Unknown phase: $PHASE (try: help)" >&2
    exit 1
    ;;
esac
