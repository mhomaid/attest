#!/usr/bin/env bash
# Bring the Railway production stack up or down.
# Workbench (https://attest.homaid.dev) stays up — marketing + waitlist.
#
#   ./scripts/prod-railway.sh down    # stop everything except workbench
#   ./scripts/prod-railway.sh up      # infra first, then apps
#   ./scripts/prod-railway.sh status
#
# Make:  make prod down | make prod up | make prod status
# Aliases: pause → down, resume → up
set -euo pipefail

PROJECT="${RAILWAY_PROJECT:-f124e2c1-3afb-49fb-9f8a-5ded89f7fbb6}"
ENVIRONMENT="${RAILWAY_ENVIRONMENT:-production}"
export RAILWAY_CALLER="${RAILWAY_CALLER:-skill:use-railway@1.2.0}"
export RAILWAY_AGENT_SESSION="${RAILWAY_AGENT_SESSION:-railway-skill-attest-prod}"

KEEP=(workbench)

# GitHub-connected app services. A push to main redeploys these unless
# watchPatterns is pinned (see guard_auto_deploy).
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
  db-migrate
)

# Image / managed services. Start these before apps.
INFRA=(
  Postgres
  minio
  redpanda
  clickhouse
  risingwave
  arroyo
  minio-init
)

CMD="${1:-}"

usage() {
  echo "Usage: $0 up|down|status"
  echo "  down    Stop every service except workbench (marketing + waitlist)."
  echo "  up      Redeploy infra, then apps. Workbench is left as-is."
  echo "  status  Print Railway deployment state for each service."
  echo
  echo "Make: make prod down | make prod up | make prod status"
  echo "Kept live: ${KEEP[*]}"
  exit 2
}

rw() {
  railway "$@" -p "$PROJECT" -e "$ENVIRONMENT"
}

is_kept() {
  local name="$1" k
  for k in "${KEEP[@]}"; do
    [[ "$name" == "$k" ]] && return 0
  done
  return 1
}

down_services() {
  local svc
  for svc in "$@"; do
    if is_kept "$svc"; then
      echo "  ↳ skip $svc (must stay live)"
      continue
    fi
    echo "  → down $svc"
    rw down --service "$svc" --yes || true
  done
}

resume_services() {
  local svc
  for svc in "$@"; do
    echo "  → up $svc"
    if rw redeploy --service "$svc" --from-source --yes; then
      continue
    fi
    echo "    ↳ from-source failed — trying latest snapshot"
    rw redeploy --service "$svc" --yes || \
      echo "    ↳ redeploy skipped for $svc (no prior deploy)"
  done
}

# Stop a git push from waking parked services. Workbench keeps default watch.
guard_auto_deploy() {
  echo "▶ Pinning non-workbench GitHub services to manual deploy only"
  local args=() svc
  for svc in "${APPS[@]}"; do
    args+=(--service-config "$svc" build.watchPatterns '[".railway-manual-only"]')
  done
  rw environment edit "${args[@]}" -m "park: only workbench auto-deploys from git" || \
    echo "    ↳ watchPatterns edit failed — next git push may still wake app services"
}

status_services() {
  local svc
  echo "project=$PROJECT  environment=$ENVIRONMENT"
  echo "keep live: ${KEEP[*]}"
  echo
  for svc in "${KEEP[@]}" "${INFRA[@]}" "${APPS[@]}"; do
    printf '%-22s ' "$svc"
    rw service status --service "$svc" --json 2>/dev/null \
      | python3 -c 'import json,sys
try:
  d=json.load(sys.stdin)
  dep=(d.get("deployment") or d.get("latestDeployment") or {})
  print(dep.get("status") or d.get("status") or "?")
except Exception:
  print("unknown")' \
      || echo "unknown"
  done
}

case "$CMD" in
  down|pause)
    echo "▶ Taking Railway demo down. Workbench stays up."
    down_services "${APPS[@]}" "${INFRA[@]}"
    guard_auto_deploy
    echo "✔ Down. Site: https://attest.homaid.dev"
    echo "  Bring the stack back:  make prod up"
    echo "  Volumes still bill until deleted (postgres/clickhouse/risingwave/orchestrator/redpanda/minio)."
    ;;
  up|resume)
    echo "▶ Bringing Railway demo up (infra first, then apps)."
    resume_services "${INFRA[@]}"
    echo "  ↳ infra triggered. Kafka / RisingWave need ~45s before apps are useful."
    resume_services "${APPS[@]}"
    echo "✔ Up triggered. Workbench: https://attest.homaid.dev"
    echo "  Check: make prod status"
    ;;
  status)
    status_services
    ;;
  *)
    usage
    ;;
esac
