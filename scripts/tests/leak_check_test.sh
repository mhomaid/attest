#!/usr/bin/env bash
# Proves check-org-leaks.sh fails closed on a planted term and on a real-looking
# AWS account id, passes the public placeholder account, and does not print the
# matched text (file:line only). Also checks the current tree against an empty list.

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
SRC="$ROOT/.cursor/skills/attest-upstream-hygiene/scripts/check-org-leaks.sh"
[[ -f "$SRC" ]] || { echo "missing $SRC" >&2; exit 1; }

TERM="ZZPLANTED9F3A"
BAD_ACCOUNT="210987654321"
PLACEHOLDER="123456789012"
REPEATED="111111111111"
failures=0

fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

new_repo() {
  local dir
  dir="$(mktemp -d "${TMPDIR:-/tmp}/attest-leak.XXXXXX")"
  git -C "$dir" init -q
  git -C "$dir" config user.email "leak-test@example.com"
  git -C "$dir" config user.name "Leak Test"
  git -C "$dir" config commit.gpgsign false
  printf '%s\n' "$dir"
}

commit_file() {
  local dir="$1" name="$2" body="$3"
  printf '%s\n' "$body" > "$dir/$name"
  git -C "$dir" add "$name"
  git -C "$dir" commit -q -m "fixture $name"
}

run_in() {
  # run_in <dir> -- env-args...
  local dir="$1"
  shift
  local out="$dir/stdout" err="$dir/stderr" rc
  set +e
  ( cd "$dir" && env -u CI -u ATTEST_ORG_DENYLIST -u ATTEST_LEAK_CHECK_OPTIONAL "$@" "$SRC" >"$out" 2>"$err" )
  rc=$?
  set -e
  printf '%s' "$rc"
}

assert_rc() {
  local label="$1" got="$2" want="$3"
  if [[ "$got" != "$want" ]]; then
    fail "$label: exit $got, want $want"
  else
    echo "ok  $label (exit $got)"
  fi
}

assert_locations_only() {
  local label="$1" file="$2" secret="$3"
  if grep -qF -- "$secret" "$file"; then
    fail "$label: matched text was printed"
    return
  fi
  if ! grep -qE '^   [^[:space:]]+:[0-9]+$' "$file"; then
    fail "$label: expected a file:line hit"
    return
  fi
  local line
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    case "$line" in
      "HIT ["*"]"|org-leak-check:*) continue ;;
    esac
    if ! printf '%s\n' "$line" | grep -qE '^   [^[:space:]]+:[0-9]+$'; then
      fail "$label: unexpected output line: $line"
      return
    fi
  done < "$file"
  echo "ok  $label (file:line only)"
}

echo "── planted deny-list term"
repo="$(new_repo)"
commit_file "$repo" "notes.txt" "ticket mentions $TERM in a comment"
rc="$(run_in "$repo" ATTEST_ORG_DENYLIST="$TERM")"
assert_rc "planted term" "$rc" 1
assert_locations_only "planted term" "$repo/stdout" "$TERM"
rm -rf "$repo"

echo "── real-looking AWS account id"
repo="$(new_repo)"
commit_file "$repo" "iam.txt" "arn:aws:s3:us-east-1:${BAD_ACCOUNT}:accesspoint/example"
rc="$(run_in "$repo" ATTEST_ORG_DENYLIST=$'# empty\n')"
assert_rc "bad account id" "$rc" 1
assert_locations_only "bad account id" "$repo/stdout" "$BAD_ACCOUNT"
rm -rf "$repo"

echo "── placeholder and repeated-digit account ids"
repo="$(new_repo)"
commit_file "$repo" "iam.txt" "arn:aws:s3:us-east-1:${PLACEHOLDER}:accesspoint/example
arn:aws:s3:us-east-1:${REPEATED}:accesspoint/example"
rc="$(run_in "$repo" ATTEST_ORG_DENYLIST=$'# empty\n')"
assert_rc "placeholder account" "$rc" 0
if grep -qE "${PLACEHOLDER}|${REPEATED}" "$repo/stdout"; then
  fail "placeholder account: account id was printed"
else
  echo "ok  placeholder account (no hit)"
fi
rm -rf "$repo"

echo "── private key header"
repo="$(new_repo)"
# Build the header in pieces so this tracked file does not itself match the check.
header="$(printf '%s%s%s' '-----BEGIN ' 'PRIVATE KEY' '-----')"
commit_file "$repo" "key.txt" "${header}
not-a-real-key
-----END-----"
rc="$(run_in "$repo" ATTEST_ORG_DENYLIST=$'# empty\n')"
assert_rc "private key" "$rc" 1
assert_locations_only "private key" "$repo/stdout" "PRIVATE KEY"
rm -rf "$repo"

echo "── CI with no deny-list fails; fork PRs do not"
repo="$(new_repo)"
commit_file "$repo" "readme.txt" "hello"
set +e
( cd "$repo" && env -u ATTEST_ORG_DENYLIST CI=true "$SRC" >"$repo/stdout" 2>"$repo/stderr" )
rc=$?
set -e
assert_rc "ci missing denylist" "$rc" 1
set +e
( cd "$repo" && env -u ATTEST_ORG_DENYLIST CI=true ATTEST_LEAK_CHECK_OPTIONAL=1 "$SRC" >"$repo/stdout" 2>"$repo/stderr" )
rc=$?
set -e
assert_rc "fork pr optional denylist" "$rc" 0
rm -rf "$repo"

echo "── current tree, empty deny-list, outside CI"
set +e
out="$(cd "$ROOT" && env -u CI ATTEST_ORG_DENYLIST=$'# empty\n' "$SRC" 2>&1)"
rc=$?
set -e
assert_rc "current tree" "$rc" 0
printf '%s\n' "$out" | grep -q "org-leak-check: PASS" || fail "current tree: missing PASS line"

echo "── workflow, gitignore, and docs"
wf="$ROOT/.github/workflows/org-leak-check.yml"
grep -q 'pull_request:' "$wf" || fail "workflow does not run on pull_request"
grep -q 'ATTEST_LEAK_CHECK_OPTIONAL' "$wf" || fail "workflow missing fork opt-out"
grep -q 'head.repo.fork' "$wf" || fail "workflow missing fork detection"
grep -q 'scripts/tests/leak_check_test.sh' "$wf" || fail "workflow does not run this test"
grep -qxF '.attest-denylist' "$ROOT/.gitignore" || fail ".attest-denylist is not gitignored"
grep -q 'check-org-leaks.sh' "$ROOT/CONTRIBUTING.md" || fail "CONTRIBUTING.md does not explain the check"
grep -q '.git/hooks/pre-commit' "$ROOT/CONTRIBUTING.md" || fail "CONTRIBUTING.md does not document the pre-commit hook"
echo "ok  workflow, gitignore, and docs"

if [[ "$failures" -ne 0 ]]; then
  echo "leak_check_test: FAIL ($failures)"
  exit 1
fi
echo "leak_check_test: PASS"
