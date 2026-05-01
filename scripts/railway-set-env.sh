#!/usr/bin/env bash
# Read each application service's environmentVariables from infra/railway/<svc>.json
# and apply them to Railway via `railway environment edit --service-config`.
# Idempotent — safe to re-run.

set -euo pipefail

SERVICES=(collector control-plane storage-iceberg detection-runtime workbench)

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Build one big argv for `railway environment edit` so we get a single commit.
args=()

for svc in "${SERVICES[@]}"; do
  config_file="infra/railway/${svc}.json"
  if [[ ! -f "$config_file" ]]; then
    echo "  ⚠ skipping $svc (no $config_file)" >&2
    continue
  fi

  # Iterate every key in environmentVariables.
  while IFS=$'\t' read -r key value; do
    [[ -z "$key" ]] && continue
    args+=(--service-config "$svc" "variables.${key}.value" "$value")
    echo "  $svc.$key"
  done < <(jq -r '.environmentVariables // {} | to_entries[] | "\(.key)\t\(.value)"' "$config_file")
done

if [[ ${#args[@]} -eq 0 ]]; then
  echo "  (no env vars to set)"
  exit 0
fi

railway environment edit "${args[@]}" -m "set application env vars from infra/railway/*.json"
