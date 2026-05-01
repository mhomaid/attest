#!/usr/bin/env bash
# deploy-pipelines.sh
# ---------------------------------------------------------------------------
# Deploys Arroyo SQL pipelines via the Arroyo REST API.
#
# Usage (local):
#   ARROYO_API=http://localhost:5115 bash infra/arroyo/deploy-pipelines.sh
#
# Environment variables:
#   ARROYO_API     Base URL of the Arroyo API  (default: http://arroyo:5115)
#   PIPELINES_DIR  Directory containing *.sql files (default: /pipelines)
#
# Arroyo auto-starts pipelines on creation, so this script:
#   1. Waits for Arroyo to be reachable.
#   2. For each *.sql file:
#      - If no pipeline with that name exists  → POST (create + auto-start)
#      - If a pipeline already exists          → PATCH stop + DELETE + re-create
#        (Arroyo does not support UPDATE query on running pipelines)
# ---------------------------------------------------------------------------
set -euo pipefail

ARROYO_API="${ARROYO_API:-http://arroyo:5115}"
PIPELINES_DIR="${PIPELINES_DIR:-/pipelines}"

GREEN=$'\033[0;32m'; YELLOW=$'\033[0;33m'; RED=$'\033[0;31m'; NC=$'\033[0m'
info()  { echo "${GREEN}[deploy-pipelines]${NC} $*"; }
warn()  { echo "${YELLOW}[deploy-pipelines]${NC} $*"; }
error() { echo "${RED}[deploy-pipelines]${NC} $*" >&2; }

# ---------------------------------------------------------------------------
wait_for_arroyo() {
    local max_attempts=30 attempt=0
    info "Waiting for Arroyo at ${ARROYO_API} ..."
    until curl -sf "${ARROYO_API}/api/v1/ping" >/dev/null 2>&1; do
        attempt=$((attempt + 1))
        [[ $attempt -ge $max_attempts ]] && { error "Arroyo did not become ready."; exit 1; }
        warn "  attempt ${attempt}/${max_attempts} — retrying in 3s"
        sleep 3
    done
    info "Arroyo is ready."
}

# Return the pipeline id for the given name, or empty string if not found.
get_pipeline_id() {
    local name="$1"
    curl -sf "${ARROYO_API}/api/v1/pipelines" \
    | python3 -c "
import sys, json
for p in json.load(sys.stdin).get('data', []):
    if p.get('name') == '${name}':
        print(p['id'])
        break
" 2>/dev/null || true
}

# Stop and delete an existing pipeline (best-effort; errors are non-fatal).
delete_pipeline() {
    local pid="$1"
    warn "  Stopping existing pipeline ${pid} ..."
    curl -sf -X PATCH "${ARROYO_API}/api/v1/pipelines/${pid}" \
        -H "Content-Type: application/json" \
        -d '{"stop":"immediate"}' >/dev/null 2>&1 || true
    sleep 3
    curl -sf -X DELETE "${ARROYO_API}/api/v1/pipelines/${pid}" >/dev/null 2>&1 || true
}

# Create (and auto-start) a pipeline from a SQL file.
# Before submitting, perform env-var substitution so the same SQL files
# work locally (redpanda:9092, minio:9000) and on Railway
# (redpanda.railway.internal:9092, minio.railway.internal:9000).
create_pipeline() {
    local sql_file="$1"
    local pipeline_name
    pipeline_name="$(basename "${sql_file}" .sql)"

    # Resolve runtime hostnames from env vars with local-dev fallbacks.
    local kafka_brokers="${KAFKA_BROKERS:-redpanda:9092}"
    local s3_endpoint_internal
    # S3_ENDPOINT may be a full URL (http://minio:9000) or just a host:port.
    # We need host:port for use inside the SQL path literal.
    if [[ "${S3_ENDPOINT:-}" =~ ^https?:// ]]; then
        s3_endpoint_internal="${S3_ENDPOINT#http://}"
        s3_endpoint_internal="${s3_endpoint_internal#https://}"
    else
        s3_endpoint_internal="${S3_ENDPOINT:-minio:9000}"
    fi

    local payload
    payload=$(python3 - <<PYEOF
import json, re

sql = open("${sql_file}").read()

# Substitute Kafka bootstrap_servers
sql = re.sub(
    r"bootstrap_servers\s*=\s*'[^']*'",
    "bootstrap_servers = '${kafka_brokers}'",
    sql
)

# Substitute MinIO / S3 endpoint in filesystem sink path
# Handles: s3::http://minio:9000/... and http://minio:9000/...
sql = re.sub(
    r"(s3::|http://)minio:9000",
    r"\g<1>${s3_endpoint_internal}",
    sql
)

print(json.dumps({
    "name": "${pipeline_name}",
    "query": sql,
    "parallelism": 1,
    "checkpoint_interval_micros": 10000000
}))
PYEOF
)

    local response
    response=$(curl -sf -X POST "${ARROYO_API}/api/v1/pipelines" \
        -H "Content-Type: application/json" \
        -d "${payload}" 2>&1) || {
        error "  curl POST failed for ${pipeline_name}: ${response}"
        return 1
    }

    local pipeline_id
    pipeline_id=$(python3 -c "import json,sys; print(json.loads(sys.argv[1])['id'])" "${response}" 2>/dev/null) || {
        error "  Failed to parse pipeline id from response: ${response}"
        return 1
    }

    info "  Created and started pipeline '${pipeline_name}' (id=${pipeline_id})"
}

# ---------------------------------------------------------------------------
deploy_pipeline() {
    local sql_file="$1"
    local pipeline_name
    pipeline_name="$(basename "${sql_file}" .sql)"

    info "Deploying pipeline: ${pipeline_name}"

    local existing_id
    existing_id=$(get_pipeline_id "${pipeline_name}")

    if [[ -n "$existing_id" ]]; then
        warn "  Pipeline '${pipeline_name}' already exists (id=${existing_id}) — replacing..."
        delete_pipeline "$existing_id"
    fi

    create_pipeline "$sql_file"
}

# ---------------------------------------------------------------------------
wait_for_arroyo

info "Scanning ${PIPELINES_DIR} for *.sql pipeline files..."
deployed=0; failed=0

for sql_file in "${PIPELINES_DIR}"/*.sql; do
    [[ -f "$sql_file" ]] || { warn "No .sql files found in ${PIPELINES_DIR}."; break; }
    if deploy_pipeline "$sql_file"; then
        deployed=$((deployed + 1))
    else
        failed=$((failed + 1))
    fi
done

echo ""
info "Done. Deployed: ${deployed}  Failed: ${failed}"
[[ $failed -eq 0 ]] || exit 1
