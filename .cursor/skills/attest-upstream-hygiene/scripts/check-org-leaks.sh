#!/usr/bin/env bash
# check-org-leaks.sh — fail if organization-specific terms appear in the public repo.
#
# The deny-list itself is private, so it is never committed here. It is read from:
#   1. $ATTEST_ORG_DENYLIST   (newline-separated; set as a GitHub Actions secret in CI)
#   2. ./.attest-denylist      (local file, gitignored; one term or regex per line, # comments)
#
# Scans tracked + staged + untracked-not-ignored files, and the latest commit message.
# Lines are case-insensitive extended regexes. Exit 1 on any hit.
#
# Also always checks a few generic patterns that are never right in this repo
# (real-looking AWS account IDs in ARNs, private keys).

set -uo pipefail
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT" || exit 2

TERMS=""
if [[ -n "${ATTEST_ORG_DENYLIST:-}" ]]; then
  TERMS="$ATTEST_ORG_DENYLIST"
elif [[ -f .attest-denylist ]]; then
  TERMS="$(cat .attest-denylist)"
fi
TERMS="$(printf '%s\n' "$TERMS" | sed -e 's/#.*$//' -e 's/[[:space:]]*$//' | grep -v '^[[:space:]]*$' || true)"

if [[ -z "$TERMS" ]]; then
  if [[ "${CI:-}" == "true" && "${ATTEST_LEAK_CHECK_OPTIONAL:-}" != "1" ]]; then
    echo "org-leak-check: no deny-list provided (set ATTEST_ORG_DENYLIST secret)" >&2
    exit 1
  fi
  echo "org-leak-check: WARNING no deny-list found (.attest-denylist or ATTEST_ORG_DENYLIST); running generic checks only" >&2
fi

FILES="$( { git ls-files; git diff --name-only --cached; git ls-files --others --exclude-standard; } 2>/dev/null \
  | sort -u \
  | grep -vE '(^|/)(Cargo\.lock|bun\.lock|uv\.lock|package-lock\.json)$' \
  | grep -vE '\.(png|jpg|jpeg|gif|ico|onnx|parquet|gz|zip|pdf|woff2?)$' \
  | grep -v '^\.attest-denylist$' )"

HITS=0

scan() {
  # scan <label> <regex>
  local label="$1" re="$2" out
  out="$(printf '%s\n' "$FILES" | while IFS= read -r f; do [[ -f "$f" ]] && printf '%s\0' "$f"; done \
        | xargs -0 grep -nIiE -- "$re" 2>/dev/null | cut -d: -f1,2 | head -20)"
  # only file:line is printed, never the matched text, so public CI logs do not republish it
  if [[ -n "$out" ]]; then
    echo "HIT [$label]"
    echo "$out" | sed 's/^/   /'
    HITS=$((HITS + 1))
  fi
}

# Generic: AWS ARNs or ECR hosts with a real-looking account ID.
# Placeholders allowed: 123456789012 (AWS docs) and repeated digits like 111111111111.
acct_out="$(printf '%s\n' "$FILES" | while IFS= read -r f; do [[ -f "$f" ]] && printf '%s\0' "$f"; done \
      | xargs -0 grep -nIoE 'arn:aws[a-z-]*:[a-z0-9-]+:[a-z0-9-]*:[0-9]{12}:|[0-9]{12}\.dkr\.ecr\.' 2>/dev/null \
      | grep -vE '123456789012|([0-9])\1{11}' | cut -d: -f1,2 | head -20)"
if [[ -n "$acct_out" ]]; then
  echo "HIT [aws-account-id]"
  echo "$acct_out" | sed 's/^/   /'
  HITS=$((HITS + 1))
fi
scan "private-key" '-----BEGIN ([A-Z]+ )?PRIVATE KEY-----'

# Private deny-list
if [[ -n "$TERMS" ]]; then
  n=0
  while IFS= read -r term; do
    n=$((n + 1))
    # label hides the term itself so CI logs do not republish it
    scan "denylist#$n" "$term"
  done <<<"$TERMS"
  # latest commit message too
  if git log -1 --format=%B >/dev/null 2>&1; then
    msg="$(git log -1 --format=%B)"
    n=0
    while IFS= read -r term; do
      n=$((n + 1))
      if grep -qiE -- "$term" <<<"$msg"; then echo "HIT [denylist#$n] in last commit message"; HITS=$((HITS + 1)); fi
    done <<<"$TERMS"
  fi
fi

if [[ $HITS -gt 0 ]]; then
  echo "org-leak-check: FAIL ($HITS pattern(s) matched). Remove the content; do not allow-list it."
  exit 1
fi
echo "org-leak-check: PASS"
