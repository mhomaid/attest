#!/usr/bin/env bash
# Every ATTEST_* key named in the single-org deploy guide must exist in
# env.example or the Helm chart. Comments count: the key is documented.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
doc="$root/docs/17_Deploy_Single_Org.md"
haystack="$(cat "$root/env.example" "$root/infra/helm/attest/values.yaml" "$root/infra/helm/attest/templates/configmap.yaml")"
missing=0
while IFS= read -r key; do
  if ! printf '%s\n' "$haystack" | grep -q -- "$key"; then
    echo "deploy guide key missing from env or Helm: $key" >&2
    missing=1
  fi
done < <(grep -oE 'ATTEST_[A-Z0-9_]+' "$doc" | sort -u)
if [[ "$missing" -ne 0 ]]; then
  exit 1
fi
echo "ok: deploy guide keys are documented"
