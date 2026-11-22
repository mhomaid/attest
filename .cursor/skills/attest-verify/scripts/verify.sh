#!/usr/bin/env bash
# verify.sh — the gate the work loop uses before calling anything done.
#
#   verify.sh quick   fast checks scoped to what changed (use while iterating)
#   verify.sh full    everything CI runs that can run locally (use before marking done)
#
# Exit 0 only if every check that ran passed. In full mode a missing toolchain is a failure.
# Last line is always a one-line summary to paste into TASKS.md evidence.

set -uo pipefail

MODE="${1:-quick}"
case "$MODE" in quick|full) ;; *) echo "usage: $0 quick|full" >&2; exit 2 ;; esac

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT" || exit 2

PASSED=()
FAILED=()
SKIPPED=()

run() {
  # run <name> <command...>
  local name="$1"; shift
  echo
  echo "── $name"
  echo "   \$ $*"
  if "$@"; then
    PASSED+=("$name")
  else
    FAILED+=("$name")
  fi
}

need() {
  # need <tool> <check-name>; returns 1 (and records) if tool is missing
  if command -v "$1" >/dev/null 2>&1; then return 0; fi
  if [[ "$MODE" == "full" ]]; then
    echo "── $2: '$1' not installed (required in full mode)"
    FAILED+=("$2 (missing $1)")
  else
    SKIPPED+=("$2 (no $1)")
  fi
  return 1
}

# ── What changed? (against merge-base with the default branch, plus the working tree)
BASE="$(git merge-base HEAD origin/main 2>/dev/null || git rev-parse HEAD~1 2>/dev/null || echo HEAD)"
CHANGED="$( { git diff --name-only "$BASE" 2>/dev/null; git diff --name-only 2>/dev/null; git diff --name-only --cached 2>/dev/null; git ls-files --others --exclude-standard 2>/dev/null; } | sort -u )"

changed() { grep -qE "$1" <<<"$CHANGED"; }

RUST=0; TS=0; PY=0; INFRA=0; ATT=0; ML=0; HELIQL=0
if [[ "$MODE" == "full" ]]; then RUST=1; TS=1; PY=1; ATT=1; ML=1; HELIQL=1; fi
changed '^(crates|apps/workbench-api|apps/ws-gateway|tests|tools)/|Cargo\.(toml|lock)$' && RUST=1
changed '^apps/workbench/|^package\.json$|^bun\.lock$' && TS=1
changed '^ml/|^eval/' && PY=1 && ML=1
changed '^(infra|docker-compose\.yml)' && INFRA=1
changed '^crates/(attest-attestation|attest-cli)/|^examples/verify/' && ATT=1
changed '^crates/(attest-onnx-runtime|attest-feature-extractor)/' && ML=1
changed '^crates/attest-heliql/|^detections/' && HELIQL=1

echo "verify.sh mode=$MODE  base=${BASE:0:12}"
echo "areas: rust=$RUST ts=$TS py=$PY infra=$INFRA attestation=$ATT ml=$ML heliql=$HELIQL"

# ── Always: org hygiene
HYG="$ROOT/.cursor/skills/attest-upstream-hygiene/scripts/check-org-leaks.sh"
if [[ -x "$HYG" ]]; then run "org-leak-check" "$HYG"; else FAILED+=("org-leak-check (script missing)"); fi

# ── Rust
if [[ $RUST -eq 1 ]] && need cargo "rust"; then
  run "cargo fmt" cargo fmt --all -- --check
  if [[ "$MODE" == "full" ]]; then
    run "cargo clippy" cargo clippy --workspace --all-targets -- -D warnings
    run "cargo test" cargo test --workspace
  else
    # scope to changed crates when possible
    PKGS=()
    while IFS= read -r d; do
      [[ -f "$d/Cargo.toml" ]] || continue
      p="$(grep -m1 -E '^name *= *"' "$d/Cargo.toml" | sed -E 's/.*"(.*)".*/\1/')"
      [[ -n "$p" ]] && PKGS+=("-p" "$p")
    done < <(grep -oE '^(crates|apps|tools|tests)/[^/]+' <<<"$CHANGED" | sort -u)
    if [[ ${#PKGS[@]} -gt 0 ]]; then
      run "cargo clippy (changed)" cargo clippy "${PKGS[@]}" --all-targets -- -D warnings
      run "cargo test (changed)" cargo test "${PKGS[@]}"
    else
      run "cargo clippy" cargo clippy --workspace --all-targets -- -D warnings
      run "cargo test" cargo test --workspace
    fi
  fi
fi

# ── Classifier latency budget (release) — the P99 < 5 ms claim
if [[ $ML -eq 1 ]] && need cargo "latency budget"; then
  if [[ -f ml/triager/artifacts/model.onnx ]]; then
    run "classifier P99 budget" cargo test --release -p attest-onnx-runtime --test classifier_integration
  else
    SKIPPED+=("classifier P99 budget (no model.onnx; run make train-classifier)")
  fi
fi

# ── Attestation: offline verify example must still pass and still catch tampering
if [[ $ATT -eq 1 ]] && need cargo "attestation"; then
  run "attestation + cli tests" cargo test -p attest-attestation -p attest-cli
  if [[ -x examples/verify/run.sh ]]; then
    run "offline verify example" examples/verify/run.sh
  fi
fi

# ── HELIQL / detections
if [[ $HELIQL -eq 1 ]] && need cargo "heliql"; then
  run "heliql tests" cargo test -p attest-heliql
fi

# ── TypeScript (workbench)
if [[ $TS -eq 1 ]] && need bun "typescript"; then
  run "bun typecheck" bun run typecheck
  run "bun lint" bun run lint
  run "bun test" bun run test
fi

# ── Python (ml)
if [[ $PY -eq 1 ]] && need uv "python"; then
  run "pytest (ml)" bash -c 'cd ml && env -u VIRTUAL_ENV uv run pytest datasets/ triager/ test_smoke.py -q'
fi

# ── Infra
if [[ $INFRA -eq 1 ]]; then
  if need helm "helm"; then run "helm template" make helm-template; fi
  if need terraform "terraform"; then run "terraform validate" make tf-validate; fi
  if [[ "$MODE" == "full" ]] && need docker "compose smoke"; then
    run "compose config" docker compose config -q
  fi
fi

echo
echo "════════════════════════════════════════"
for s in "${SKIPPED[@]:-}"; do [[ -n "$s" ]] && echo "skip  $s"; done
for f in "${FAILED[@]:-}"; do [[ -n "$f" ]] && echo "FAIL  $f"; done
TOTAL=$(( ${#PASSED[@]} + ${#FAILED[@]} ))
STAMP="$(date -u +%Y-%m-%dT%H:%MZ)"
if [[ ${#FAILED[@]} -eq 0 && $TOTAL -gt 0 ]]; then
  echo "VERIFY PASS mode=$MODE checks=${#PASSED[@]}/$TOTAL skipped=${#SKIPPED[@]} commit=$(git rev-parse --short HEAD 2>/dev/null) at=$STAMP"
  exit 0
else
  echo "VERIFY FAIL mode=$MODE failed=${#FAILED[@]} passed=${#PASSED[@]} commit=$(git rev-parse --short HEAD 2>/dev/null) at=$STAMP"
  exit 1
fi
