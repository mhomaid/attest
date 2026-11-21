#!/usr/bin/env bash
# Proves scripts/check-dco.sh rejects a commit without Signed-off-by and accepts one with it.

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
SRC="$ROOT/scripts/check-dco.sh"
[[ -f "$SRC" ]] || { echo "missing $SRC" >&2; exit 1; }

new_repo() {
  local dir
  dir="$(mktemp -d "${TMPDIR:-/tmp}/attest-dco.XXXXXX")"
  git -C "$dir" init -q
  git -C "$dir" config user.email "dco-test@example.com"
  git -C "$dir" config user.name "DCO Test"
  git -C "$dir" config commit.gpgsign false
  git -C "$dir" commit -q --allow-empty -m "base"
  git -C "$dir" rev-parse HEAD
  printf '%s\n' "$dir"
}

echo "── unsigned commit"
base_and_dir="$(new_repo)"
base="$(printf '%s\n' "$base_and_dir" | head -n 1)"
repo="$(printf '%s\n' "$base_and_dir" | tail -n 1)"
git -C "$repo" commit -q --allow-empty -m "missing sign-off"
set +e
out="$(cd "$repo" && "$SRC" "$base" 2>&1)"
rc=$?
set -e
if [[ "$rc" -eq 1 ]] && printf '%s\n' "$out" | grep -q 'missing Signed-off-by'; then
  echo "ok  unsigned commit (exit 1)"
else
  echo "FAIL: unsigned commit: exit $rc" >&2
  printf '%s\n' "$out" >&2
  rm -rf "$repo"
  exit 1
fi
rm -rf "$repo"

echo "── signed commit"
base_and_dir="$(new_repo)"
base="$(printf '%s\n' "$base_and_dir" | head -n 1)"
repo="$(printf '%s\n' "$base_and_dir" | tail -n 1)"
git -C "$repo" commit -q --allow-empty -m "signed

Signed-off-by: DCO Test <dco-test@example.com>"
set +e
out="$(cd "$repo" && "$SRC" "$base" 2>&1)"
rc=$?
set -e
if [[ "$rc" -eq 0 ]] && printf '%s\n' "$out" | grep -q 'dco-check: PASS'; then
  echo "ok  signed commit (exit 0)"
else
  echo "FAIL: signed commit: exit $rc" >&2
  printf '%s\n' "$out" >&2
  rm -rf "$repo"
  exit 1
fi
rm -rf "$repo"

echo "dco_check_test: PASS"
