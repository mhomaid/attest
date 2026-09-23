#!/usr/bin/env bash
# Pause, resume, or take down the Attest Railway production demo.
# Workbench (marketing site) and Postgres stay up so the homepage and
# auth/seed data survive a pause or down.
#
# Railway `scale REGION=0` unassigns the region instead of parking a replica,
# so pause/down use `railway down` (remove the active deployment).
# Resume redeploys from the last uploaded source / configured image.
set -euo pipefail

PROJECT="${RAILWAY_PROJECT:-f124e2c1-3afb-49fb-9f8a-5ded89f7fbb6}"
ENVIRONMENT="${RAILWAY_ENVIRONMENT:-production}"
export RAILWAY_CALLER="${RAILWAY_CALLER:-skill:use-railway@1.2.0}"
export RAILWAY_AGENT_SESSION="${RAILWAY_AGENT_SESSION:-railway-skill-attest-prod}"

# Workbench is the public marketing site; never taken down by pause/down.
APPS=(
  collector
  control-plane
  storage-iceberg
  detection-runtime
  orchestrator
  mcp-gateway
  calibration-sidecar
  workbench-api
  arroyo-deployer
)
INFRA=(minio redpanda clickhouse risingwave arroyo)
CMD="${1:-}"

usage() {
  echo "Usage: $0 pause|resume|down"
  echo "  pause   Stop demo compute (railway down). Workbench + Postgres stay up."
  echo "  resume  Redeploy each service from its last source/image."
  echo "  down    Same stop as pause — explicit take-offline name."
  exit 2
}

rw() {
  railway "$@" -p "$PROJECT" -e "$ENVIRONMENT"
}

down_services() {
  local svc
  for svc in "$@"; do
    echo "  → down $svc"
    rw down --service "$svc" --yes || true
  done
}

resume_services() {
  local svc
  for svc in "$@"; do
    echo "  → resume $svc"
    if rw redeploy --service "$svc" --from-source --yes; then
      continue
    fi
    echo "    ↳ from-source failed — trying latest snapshot"
    rw redeploy --service "$svc" --yes || \
      echo "    ↳ redeploy skipped for $svc (no prior deploy)"
  done
}

case "$CMD" in
  pause)
    echo "▶ Pausing Railway demo. Workbench + Postgres stay up."
    down_services "${APPS[@]}" "${INFRA[@]}"
    echo "✔ Paused. Marketing site: https://attest.homaid.dev"
    echo "  Resume the full stack with: make prod resume"
    ;;
  resume)
    echo "▶ Resuming Railway demo (infra first, then apps)."
    resume_services "${INFRA[@]}"
    resume_services "${APPS[@]}"
    echo "✔ Resume triggered. Workbench: https://attest-wb.up.railway.app"
    ;;
  down)
    echo "▶ Taking Railway demo down. Workbench + Postgres stay up."
    down_services "${APPS[@]}" "${INFRA[@]}"
    echo "✔ Down. Marketing site: https://attest.homaid.dev"
    echo "  Bring the full stack back with: make prod resume"
    ;;
  *)
    usage
    ;;
esac
