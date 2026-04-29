#!/usr/bin/env bash
# deploy-pipelines.sh
# ---------------------------------------------------------------------------
# Deploys Arroyo SQL pipelines via the Arroyo REST API.
#
# Usage:
#   ARROYO_API=http://localhost:5115  ./infra/arroyo/deploy-pipelines.sh
#
# Environment variables:
#   ARROYO_API   Base URL of the Arroyo API  (default: http://arroyo:5115)
#   PIPELINES_DIR  Directory containing *.sql pipeline files
#                (default: /pipelines, the container mount point)
#
# The script is idempotent: if a pipeline with the same name already exists
# it updates (PATCH) it rather than creating a duplicate. It then starts the
# pipeline if it isn't already running.
# ---------------------------------------------------------------------------
set -euo pipefail

ARROYO_API="${ARROYO_API:-http://arroyo:5115}"
PIPELINES_DIR="${PIPELINES_DIR:-/pipelines}"

# Colour helpers (no-op when stdout is not a terminal)
RED=$'\033[0;31m'; GREEN=$'\033[0;32m'; YELLOW=$'\033[0;33m'; NC=$'\033[0m'
info()  { echo "${GREEN}[deploy-pipelines]${NC} $*"; }
warn()  { echo "${YELLOW}[deploy-pipelines]${NC} $*"; }
error() { echo "${RED}[deploy-pipelines]${NC} $*" >&2; }

# ---------------------------------------------------------------------------
# Wait for Arroyo to be reachable
# ---------------------------------------------------------------------------
wait_for_arroyo() {
    local max_attempts=30
    local attempt=0
    info "Waiting for Arroyo at ${ARROYO_API} ..."
    until curl -sf "${ARROYO_API}/api/v1/ping" >/dev/null 2>&1; do
        attempt=$((attempt + 1))
        if [[ $attempt -ge $max_attempts ]]; then
            error "Arroyo did not become ready after ${max_attempts} attempts."
            exit 1
        fi
        warn "  attempt ${attempt}/${max_attempts} — retrying in 3s"
        sleep 3
    done
    info "Arroyo is ready."
}

# ---------------------------------------------------------------------------
# Deploy a single pipeline from a .sql file
# ---------------------------------------------------------------------------
deploy_pipeline() {
    local sql_file="$1"
    local pipeline_name
    pipeline_name="$(basename "${sql_file}" .sql)"
    local sql_body
    sql_body="$(cat "${sql_file}")"

    info "Deploying pipeline: ${pipeline_name}"

    # Check whether the pipeline already exists
    local existing_id
    existing_id=$(
        curl -sf "${ARROYO_API}/api/v1/pipelines" \
        | python3 -c "
import sys, json
pipelines = json.load(sys.stdin).get('data', [])
for p in pipelines:
    if p.get('name') == '${pipeline_name}':
        print(p['id'])
        break
" 2>/dev/null || true
    )

    local payload
    payload=$(python3 -c "
import json, sys
print(json.dumps({
    'name': '${pipeline_name}',
    'query': sys.stdin.read(),
    'parallelism': 1,
    'enabledCheckpoints': True,
    'checkpointIntervalMicros': 10000000
}))" <<< "${sql_body}")

    if [[ -n "$existing_id" ]]; then
        warn "  Pipeline '${pipeline_name}' already exists (id=${existing_id}). Updating..."
        http_status=$(
            curl -s -o /dev/null -w "%{http_code}" \
                -X PATCH \
                -H "Content-Type: application/json" \
                -d "${payload}" \
                "${ARROYO_API}/api/v1/pipelines/${existing_id}"
        )
        if [[ "$http_status" != "200" ]]; then
            error "  PATCH failed (HTTP ${http_status}) for ${pipeline_name}"
            return 1
        fi
        pipeline_id="$existing_id"
    else
        info "  Creating new pipeline '${pipeline_name}'..."
        response=$(
            curl -sf \
                -X POST \
                -H "Content-Type: application/json" \
                -d "${payload}" \
                "${ARROYO_API}/api/v1/pipelines"
        )
        pipeline_id=$(python3 -c "import json,sys; print(json.load(sys.stdin)['id'])" <<< "${response}")
        info "  Created pipeline id=${pipeline_id}"
    fi

    # Start (or resume) the pipeline
    local current_state
    current_state=$(
        curl -sf "${ARROYO_API}/api/v1/pipelines/${pipeline_id}" \
        | python3 -c "import json,sys; print(json.load(sys.stdin).get('state','unknown'))" 2>/dev/null || echo "unknown"
    )

    if [[ "$current_state" == "Running" ]]; then
        info "  Pipeline '${pipeline_name}' is already Running — no action needed."
    else
        info "  Starting pipeline '${pipeline_name}' (current state: ${current_state})..."
        http_status=$(
            curl -s -o /dev/null -w "%{http_code}" \
                -X POST \
                -H "Content-Type: application/json" \
                -d '{"action":"start"}' \
                "${ARROYO_API}/api/v1/pipelines/${pipeline_id}/action"
        )
        if [[ "$http_status" == "200" || "$http_status" == "204" ]]; then
            info "  Pipeline '${pipeline_name}' started (HTTP ${http_status})."
        else
            error "  Start action failed (HTTP ${http_status}) for ${pipeline_name}"
            return 1
        fi
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
wait_for_arroyo

info "Scanning ${PIPELINES_DIR} for *.sql pipeline files..."
deployed=0
failed=0

for sql_file in "${PIPELINES_DIR}"/*.sql; do
    if [[ ! -f "$sql_file" ]]; then
        warn "No .sql files found in ${PIPELINES_DIR}."
        break
    fi
    if deploy_pipeline "$sql_file"; then
        deployed=$((deployed + 1))
    else
        failed=$((failed + 1))
    fi
done

echo ""
info "Done. Deployed: ${deployed}  Failed: ${failed}"
[[ $failed -eq 0 ]] || exit 1
