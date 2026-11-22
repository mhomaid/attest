#!/usr/bin/env bash
# Fail if any commit since the base lacks a Signed-off-by line (DCO).
# Usage: scripts/check-dco.sh [base-sha]
# In GitHub Actions, GITHUB_BASE_REF selects the PR base.

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT" || exit 2

if [[ -n "${1:-}" ]]; then
  BASE="$1"
elif [[ -n "${GITHUB_BASE_REF:-}" ]]; then
  git fetch --no-tags origin "$GITHUB_BASE_REF" >/dev/null 2>&1 || true
  BASE="$(git merge-base HEAD "origin/$GITHUB_BASE_REF" 2>/dev/null || true)"
else
  BASE="$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD main 2>/dev/null || true)"
fi

if [[ -z "${BASE:-}" ]]; then
  echo "dco-check: could not find a base commit" >&2
  exit 1
fi

# GitHub's pull_request checkout is a merge of the head into the base. That
# merge commit is created by Actions and has no Signed-off-by line.
missing=0
while IFS= read -r sha; do
  [[ -z "$sha" ]] && continue
  subject="$(git log -1 --format=%s "$sha")"
  if ! git log -1 --format=%B "$sha" | grep -q '^Signed-off-by: '; then
    echo "dco-check: $sha missing Signed-off-by ($subject)"
    missing=1
  fi
done < <(git rev-list --no-merges "${BASE}..HEAD")

if [[ "$missing" -ne 0 ]]; then
  echo "dco-check: FAIL. Create commits with git commit -s."
  exit 1
fi
echo "dco-check: PASS"
