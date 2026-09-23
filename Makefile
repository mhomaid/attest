.PHONY: help dev-up-infra dev-up-services dev-up-all dev-down-infra dev-down-all smoke seed-data fmt test lint train-classifier run-calibration run-mcp-gateway run-control-plane run-workbench-api run-orchestrator run-orchestrator-local run-orchestrator-cloud railway-login railway-setup railway-domain railway-status railway-logs railway-stop railway-infra-stop railway-infra railway-infra-config railway-infra-deploy railway-app-start railway-full-deploy prod pause resume down e2e-phase1 e2e-phase2 e2e-phase3 e2e-phase4a e2e-phase4b e2e-phase5 e2e-phase6 e2e-phase7 e2e-phase7-live e2e-run-phase e2e-all-offline e2e-all-platform arroyo-ui redpanda-ui arroyo-deploy e2e-arroyo load-gen-up load-test load-test-burst load-status load-stop load-cli-smoke load-cli-burst load-cli-attack
.DEFAULT_GOAL := help

# Source repo-root `.env` in native `make run-*` / E2E recipes below.
# Docker Compose loads `.env` by default; a bare shell does not — without this, values only exist in `.env`, not in the process environment.
DOTENV_SH := set -a; [ -f "$(CURDIR)/.env" ] && . "$(CURDIR)/.env"; set +a;

help: ## Show this help message
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n\nTargets:\n"} \
	  /^[a-zA-Z0-9_-]+:.*?##/ { printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

dev-up-infra: ## Start core infrastructure (Redpanda, RisingWave, ClickHouse, MinIO, Postgres, Redpanda Console)
	docker compose up -d redpanda redpanda-console postgres minio clickhouse risingwave
	docker compose run --rm minio-init

dev-down-infra: ## Stop core infrastructure containers only
	docker compose down

dev-down-all: ## Stop and remove ALL containers (infra + services)
	docker compose --profile platform down
	docker compose down

dev-up-services: ## Start app services only — assumes infra is already running (use dev-up-all for a fresh start)
	docker compose --profile platform up -d --build

dev-up-all: ## ⭐ Start everything in order: infra → init → all services (clean fresh start)
	docker compose up -d redpanda redpanda-console postgres minio clickhouse risingwave
	docker compose run --rm minio-init
	docker compose --profile platform up -d --build

e2e-phase1: ## Run Phase 1 E2E test — event appears in RisingWave within 5s (requires ATTEST_E2E=1 + platform)
	ATTEST_E2E=1 cargo test --test phase1_streaming -- --nocapture

e2e-phase2: ## Run Phase 2 E2E test — 10K events land in MinIO Parquet + ClickHouse query (requires ATTEST_E2E=1 + platform)
	ATTEST_E2E=1 cargo test --test phase2_iceberg -- --nocapture

e2e-phase3: ## Run Phase 3 E2E test — HELIQL detections fire alerts on stream (requires ATTEST_E2E=1 + platform)
	ATTEST_E2E=1 cargo test --test phase3_detection -- --nocapture

train-classifier: ## Train XGBoost classifier + novelty detector + calibration (outputs to ml/triager/artifacts/)
	cd ml && env -u VIRTUAL_ENV uv run python triager/train.py
	cd ml && env -u VIRTUAL_ENV uv run python triager/novelty.py
	cd ml && env -u VIRTUAL_ENV uv run python triager/calibrate.py --train

run-calibration: ## Run calibration sidecar natively (port 5001); unsets VIRTUAL_ENV so uv uses ml/.venv
	cd ml && env -u VIRTUAL_ENV CALIBRATION_PORT=5001 uv run python triager/calibrate.py --serve

run-mcp-gateway: ## Run MCP gateway natively (port 4242) — set CONTROL_PLANE_URL if not localhost:8080
	$(DOTENV_SH) \
	CONTROL_PLANE_URL=$${CONTROL_PLANE_URL:-http://localhost:8080} \
	  MCP_GATEWAY_PORT=4242 \
	  cargo run -p attest-mcp-gateway

run-control-plane: ## Run control-plane API natively (port 8080) — requires RisingWave, ClickHouse, Kafka reachable via env
	$(DOTENV_SH) \
	RISINGWAVE_HOST=$${RISINGWAVE_HOST:-localhost} \
	  RISINGWAVE_PORT=$${RISINGWAVE_PORT:-4566} \
	  CLICKHOUSE_URL=$${CLICKHOUSE_URL:-http://localhost:8123} \
	  KAFKA_BROKERS=$${KAFKA_BROKERS:-localhost:19092} \
	  PORT=8080 \
	  cargo run -p attest-control-plane

run-workbench-api: ## Run workbench trace API (port 4400) — reads ATTEST_LOG_PATH (default ./attestations.ndjson)
	$(DOTENV_SH) \
	WORKBENCH_API_PORT=$${WORKBENCH_API_PORT:-4400} \
	  ATTEST_LOG_PATH=$${ATTEST_LOG_PATH:-$(CURDIR)/attestations.ndjson} \
	  cargo run -p workbench-api

run-orchestrator: ## Run orchestrator natively (local Unsloth LLM path — requires calibration sidecar on :5001 and Unsloth Studio on :8888)
	$(DOTENV_SH) \
	ARTIFACTS_DIR=$(CURDIR)/ml/triager/artifacts \
	  ORCHESTRATOR_PORT=4300 \
	  CALIBRATION_URL=http://localhost:5001 \
	  SYSTEM_PROMPT_PATH=$(CURDIR)/agents/triager/system_prompt_v1.md \
	  REVIEWER_PROMPT_PATH=$(CURDIR)/agents/triager/reviewer_prompt_v1.md \
	  INVESTIGATOR_PROMPT_PATH=$(CURDIR)/agents/investigator/system_prompt_v1.md \
	  ATTEST_LLM_PROVIDER=local \
	  ATTEST_LLM_BASE_URL=http://127.0.0.1:8888/v1 \
	  ATTEST_LLM_MODEL=unsloth/Qwen3.6-35B-A3B-GGUF \
	  ATTEST_LLM_API_KEY=$${ATTEST_LLM_API_KEY} \
	  MCP_GATEWAY_URL=http://localhost:4242 \
	  cargo run -p attest-orchestrator

run-orchestrator-local: run-orchestrator ## Alias for run-orchestrator (local Unsloth LLM path)

run-orchestrator-cloud: ## Run orchestrator natively (Anthropic Sonnet LLM path — requires ANTHROPIC_API_KEY)
	$(DOTENV_SH) \
	test -n "$$ANTHROPIC_API_KEY" || (echo "ERROR: ANTHROPIC_API_KEY is not set (add to .env or export)"; exit 1); \
	ARTIFACTS_DIR=$(CURDIR)/ml/triager/artifacts \
	  ORCHESTRATOR_PORT=4300 \
	  CALIBRATION_URL=http://localhost:5001 \
	  SYSTEM_PROMPT_PATH=$(CURDIR)/agents/triager/system_prompt_v1.md \
	  REVIEWER_PROMPT_PATH=$(CURDIR)/agents/triager/reviewer_prompt_v1.md \
	  INVESTIGATOR_PROMPT_PATH=$(CURDIR)/agents/investigator/system_prompt_v1.md \
	  ATTEST_LLM_PROVIDER=anthropic \
	  ATTEST_LLM_MODEL=claude-sonnet-4-5 \
	  MCP_GATEWAY_URL=http://localhost:4242 \
	  cargo run -p attest-orchestrator

e2e-phase4a: ## Run Phase 4a E2E tests — starts services, runs tests, cleans up
	@echo "==> Cleaning up any previous test processes..."
	@pkill -f "target.*attest-orchestrator" 2>/dev/null || true
	@pkill -f "calibrate.py" 2>/dev/null || true
	@sleep 1
	@echo "==> Starting calibration sidecar (port 5001)..."
	cd ml && env -u VIRTUAL_ENV CALIBRATION_PORT=5001 uv run python triager/calibrate.py --serve > /tmp/attest-calibration.log 2>&1 &
	@echo "==> Starting orchestrator (port 4300)..."
	ARTIFACTS_DIR=$(CURDIR)/ml/triager/artifacts \
	  ORCHESTRATOR_PORT=4300 \
	  CALIBRATION_URL=http://localhost:5001 \
	  ATTEST_LOG_PATH=/tmp/attest-test-attestations.ndjson \
	  cargo run -q -p attest-orchestrator > /tmp/attest-orchestrator.log 2>&1 &
	@echo "==> Waiting for orchestrator to be ready (up to 60s)..."
	@for i in $$(seq 1 60); do \
	  curl -sf http://localhost:4300/healthz > /dev/null 2>&1 && echo "  ready after $${i}s" && break; \
	  sleep 1; \
	done
	@curl -sf http://localhost:4300/healthz > /dev/null 2>&1 || \
	  (echo "ERROR: orchestrator did not start. Logs:"; cat /tmp/attest-orchestrator.log; \
	   pkill -f "calibrate.py" 2>/dev/null || true; exit 1)
	@echo "==> Running Phase 4a E2E tests..."
	ATTEST_E2E=1 cargo test --test phase4a_triager -- --nocapture; \
	  STATUS=$$?; \
	  echo "==> Stopping services..."; \
	  pkill -f "target.*attest-orchestrator" 2>/dev/null || true; \
	  pkill -f "calibrate.py" 2>/dev/null || true; \
	  exit $$STATUS

e2e-phase4b: ## Run Phase 4b E2E tests — hybrid LLM escalation (requires ATTEST_LLM_PROVIDER=local and Unsloth on :8888)
	@echo "==> Cleaning up any previous test processes..."
	@pkill -f "target.*attest-orchestrator" 2>/dev/null || true
	@pkill -f "calibrate.py" 2>/dev/null || true
	@sleep 1
	@echo "==> Starting calibration sidecar (port 5001)..."
	cd ml && env -u VIRTUAL_ENV CALIBRATION_PORT=5001 uv run python triager/calibrate.py --serve > /tmp/attest-calibration.log 2>&1 &
	@echo "==> Starting orchestrator with local LLM config (port 4300)..."
	ARTIFACTS_DIR=$(CURDIR)/ml/triager/artifacts \
	  ORCHESTRATOR_PORT=4300 \
	  CALIBRATION_URL=http://localhost:5001 \
	  SYSTEM_PROMPT_PATH=$(CURDIR)/agents/triager/system_prompt_v1.md \
	  ATTEST_LLM_PROVIDER=local \
	  ATTEST_LLM_BASE_URL=http://127.0.0.1:8888/v1 \
	  ATTEST_LLM_MODEL=unsloth/Qwen3.6-35B-A3B-GGUF \
	  MCP_GATEWAY_URL=http://localhost:4242 \
	  ATTEST_LOG_PATH=/tmp/attest-test-attestations.ndjson \
	  cargo run -q -p attest-orchestrator > /tmp/attest-orchestrator.log 2>&1 &
	@echo "==> Waiting for orchestrator to be ready (up to 60s)..."
	@for i in $$(seq 1 60); do \
	  curl -sf http://localhost:4300/healthz > /dev/null 2>&1 && echo "  ready after $${i}s" && break; \
	  sleep 1; \
	done
	@curl -sf http://localhost:4300/healthz > /dev/null 2>&1 || \
	  (echo "ERROR: orchestrator did not start. Logs:"; cat /tmp/attest-orchestrator.log; \
	   pkill -f "calibrate.py" 2>/dev/null || true; exit 1)
	@echo "==> Running Phase 4b E2E tests..."
	ATTEST_E2E=1 ATTEST_LLM_PROVIDER=local cargo test --test phase4b_llm_escalation -- --nocapture; \
	  STATUS=$$?; \
	  echo "==> Stopping services..."; \
	  pkill -f "target.*attest-orchestrator" 2>/dev/null || true; \
	  pkill -f "calibrate.py" 2>/dev/null || true; \
	  exit $$STATUS

	open http://localhost:5115

e2e-phase5: ## Run Phase 5 E2E tests — hallucination guardrails (ATTEST_GUARDRAILS=on, requires local LLM)
	@echo "==> Cleaning up any previous test processes..."
	@pkill -f "target.*attest-orchestrator" 2>/dev/null || true
	@pkill -f "calibrate.py" 2>/dev/null || true
	@sleep 1
	@test -n "$$ATTEST_LLM_API_KEY" || (echo "WARNING: ATTEST_LLM_API_KEY is not set — Unsloth Studio may reject requests"; true)
	@echo "==> Starting calibration sidecar (port 5001)..."
	cd ml && env -u VIRTUAL_ENV CALIBRATION_PORT=5001 uv run python triager/calibrate.py --serve > /tmp/attest-calibration.log 2>&1 &
	@echo "==> Starting orchestrator with guardrails enabled (port 4300)..."
	ARTIFACTS_DIR=$(CURDIR)/ml/triager/artifacts \
	  ORCHESTRATOR_PORT=4300 \
	  CALIBRATION_URL=http://localhost:5001 \
	  SYSTEM_PROMPT_PATH=$(CURDIR)/agents/triager/system_prompt_v1.md \
	  REVIEWER_PROMPT_PATH=$(CURDIR)/agents/triager/reviewer_prompt_v1.md \
	  ATTEST_LLM_PROVIDER=local \
	  ATTEST_LLM_BASE_URL=http://127.0.0.1:8888/v1 \
	  ATTEST_LLM_MODEL=unsloth/Qwen3.6-35B-A3B-GGUF \
	  ATTEST_LLM_API_KEY=$(ATTEST_LLM_API_KEY) \
	  MCP_GATEWAY_URL=http://localhost:4242 \
	  ATTEST_GUARDRAILS=on \
	  GUARDRAIL_MAX_RETRIES=2 \
	  CROSS_REVIEW_SEVERITY_THRESHOLD=0.85 \
	  ATTEST_LOG_PATH=/tmp/attest-test-attestations-p5.ndjson \
	  cargo run -q -p attest-orchestrator > /tmp/attest-orchestrator-p5.log 2>&1 &
	@echo "==> Waiting for orchestrator to be ready (up to 60s)..."
	@for i in $$(seq 1 60); do \
	  curl -sf http://localhost:4300/healthz > /dev/null 2>&1 && echo "  ready after $${i}s" && break; \
	  sleep 1; \
	done
	@curl -sf http://localhost:4300/healthz > /dev/null 2>&1 || \
	  (echo "ERROR: orchestrator did not start. Logs:"; cat /tmp/attest-orchestrator-p5.log; \
	   pkill -f "calibrate.py" 2>/dev/null || true; exit 1)
	@echo "==> Running Phase 5 E2E tests..."
	ATTEST_E2E=1 ATTEST_GUARDRAILS=on GUARDRAIL_MAX_RETRIES=2 \
	  cargo test --test phase5_guardrails -- --nocapture; \
	  STATUS=$$?; \
	  echo "==> Stopping services..."; \
	  pkill -f "target.*attest-orchestrator" 2>/dev/null || true; \
	  pkill -f "calibrate.py" 2>/dev/null || true; \
	  exit $$STATUS

e2e-phase6: ## Run Phase 6 E2E tests — shadow check + triager auto-close
	@echo "==> Cleaning up any previous test processes..."
	@pkill -f "target.*attest-orchestrator" 2>/dev/null || true
	@pkill -f "calibrate.py" 2>/dev/null || true
	@sleep 1
	@echo "==> Starting calibration sidecar (port 5001)..."
	cd ml && env -u VIRTUAL_ENV CALIBRATION_PORT=5001 uv run python triager/calibrate.py --serve > /tmp/attest-calibration.log 2>&1 &
	@echo "==> Starting orchestrator with Phase 6 env vars (port 4300)..."
	ARTIFACTS_DIR=$(CURDIR)/ml/triager/artifacts \
	  ORCHESTRATOR_PORT=4300 \
	  CALIBRATION_URL=http://localhost:5001 \
	  SYSTEM_PROMPT_PATH=$(CURDIR)/agents/triager/system_prompt_v1.md \
	  REVIEWER_PROMPT_PATH=$(CURDIR)/agents/triager/reviewer_prompt_v1.md \
	  ATTEST_LLM_PROVIDER=local \
	  ATTEST_LLM_BASE_URL=http://127.0.0.1:8888/v1 \
	  ATTEST_LLM_MODEL=unsloth/Qwen3.6-35B-A3B-GGUF \
	  ATTEST_LLM_API_KEY=$(ATTEST_LLM_API_KEY) \
	  MCP_GATEWAY_URL=http://localhost:4242 \
	  AUTO_CLOSE_THRESHOLD=0.90 \
	  DO_NOT_TOUCH_LIST=ceo@corp.com,admin@corp.com \
	  TENANT_ALLOWS_AUTOMATION=true \
	  AUTO_CLOSE_ACTION_CLASSES=login,api_call,file_access \
	  ATTEST_GUARDRAILS=on \
	  ATTEST_LOG_PATH=/tmp/attest-test-attestations-p6.ndjson \
	  cargo run -q -p attest-orchestrator > /tmp/attest-orchestrator-p6.log 2>&1 &
	@echo "==> Waiting for orchestrator to be ready (up to 60s)..."
	@for i in $$(seq 1 60); do \
	  curl -sf http://localhost:4300/healthz > /dev/null 2>&1 && echo "  ready after $${i}s" && break; \
	  sleep 1; \
	done
	@curl -sf http://localhost:4300/healthz > /dev/null 2>&1 || \
	  (echo "ERROR: orchestrator did not start. Logs:"; cat /tmp/attest-orchestrator-p6.log; \
	   pkill -f "calibrate.py" 2>/dev/null || true; exit 1)
	@grep -q "Address already in use" /tmp/attest-orchestrator-p6.log && \
	  (echo "ERROR: port 4300 still in use — run: pkill -f attest-orchestrator"; exit 1) || true
	@echo "==> Running Phase 6 E2E tests..."
	ATTEST_E2E=1 AUTO_CLOSE_THRESHOLD=0.90 DO_NOT_TOUCH_LIST=ceo@corp.com,admin@corp.com \
	  TENANT_ALLOWS_AUTOMATION=true \
	  cargo test --test phase6_auto_close -- --nocapture; \
	  STATUS=$$?; \
	  echo "==> Stopping services..."; \
	  pkill -f "target.*attest-orchestrator" 2>/dev/null || true; \
	  pkill -f "calibrate.py" 2>/dev/null || true; \
	  exit $$STATUS

e2e-phase7: ## Phase 7 — Investigator loop (integration test; wiremock + scripted LLM)
	@echo "==> Running Phase 7 investigator integration tests..."
	cargo test -p attest-orchestrator --test investigator_loop -- --nocapture

e2e-phase7-live: ## Phase 7 — live stack: POST /triage → investigator → GET …/trace (see scripts/e2e/README.md)
	@echo "==> Phase 7 live E2E (orchestrator + workbench-api + shared ATTEST_LOG_PATH)..."
	$(DOTENV_SH) \
	ATTEST_E2E=1 ATTEST_PHASE7_LIVE=1 \
	  cargo test -p e2e-tests --test phase7_live_investigator -- --nocapture

e2e-run-phase: ## Usage: make e2e-run-phase P=7|7-live — shell driver (see scripts/e2e/README.md)
	@test -n "$(P)" || (echo "Set P to 1,2,3,4a,4b,5,6,7,7-live,arroyo,all-platform,all-offline,help"; exit 1)
	@scripts/e2e/run-phase.sh $(P)

e2e-all-offline: ## Rust workspace + Phase7 investigator test + ML pytest (no Docker)
	scripts/e2e/run-phase.sh all-offline

e2e-all-platform: ## Phases 1–3 only; requires stack + ATTEST_E2E implied by script
	scripts/e2e/run-phase.sh all-platform

redpanda-ui: ## Open Redpanda Console in the browser (http://localhost:8081)
	open http://localhost:8081

e2e-arroyo: ## Run Arroyo E2E tests — health, pipeline deploy, ETL Parquet, CEP alert (requires dev-up-services)
	ATTEST_E2E=1 cargo test --test phase_arroyo_pipelines -- --nocapture --test-threads=1

arroyo-deploy: ## Deploy Arroyo SQL pipelines to a running local Arroyo instance
	ARROYO_API=http://localhost:5115 \
	PIPELINES_DIR=$(CURDIR)/infra/arroyo/pipelines \
	bash infra/arroyo/deploy-pipelines.sh

dev-up-llm: ## Start core services + llama.cpp (requires Qwen GGUF in llama-models volume)
	docker compose --profile llm up -d

dev-up-ml: ## Start core services + Python ML sidecar
	docker compose --profile ml up -d

smoke: ## Quick sanity check: cargo test + bun test + pytest
	cargo test --workspace
	bun run test
	cd ml && env -u VIRTUAL_ENV uv run pytest triager/ test_smoke.py -q

seed-data: ## Seed ~30k synthetic CloudTrail events (benign + attack scenarios) into the running stack
	cargo run --release -p attest-load-gen -- --brokers localhost:19092 --rate 1000 --duration 30 --scenario mixed --tenants 3 --seed-baselines

fmt: ## Format all Rust code
	cargo fmt --all

test: ## Run all tests: Rust workspace + JS + Python
	cargo test --workspace
	bun run test
	cd ml && env -u VIRTUAL_ENV uv run pytest triager/ test_smoke.py -q

lint: ## Lint all code: clippy + bun lint
	cargo clippy --workspace --all-targets -- -D warnings
	bun run lint

# ── Railway deployment ────────────────────────────────────────────────────────

railway-login: ## Log in to Railway CLI
	railway login

railway-setup: ## Create all services AND configure each one's builder (Dockerfile path or Nixpacks) — fully CLI driven
	@echo "▶ Creating services (skipping if they already exist)…"
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer orchestrator mcp-gateway calibration-sidecar; do \
	  echo "  → $$svc"; \
	  railway add --service $$svc >/dev/null 2>&1 || true; \
	done
	@echo "▶ Configuring builders and start commands (will prompt for confirmation)…"
	@railway environment edit \
	  --service-config collector         build.builder        DOCKERFILE \
	  --service-config collector         build.dockerfilePath infra/docker/collector.Dockerfile \
	  --service-config collector         deploy.startCommand  "attest-collector serve" \
	  --service-config control-plane     build.builder        DOCKERFILE \
	  --service-config control-plane     build.dockerfilePath infra/docker/control-plane.Dockerfile \
	  --service-config control-plane     deploy.startCommand  "attest-control-plane" \
	  --service-config storage-iceberg   build.builder        DOCKERFILE \
	  --service-config storage-iceberg   build.dockerfilePath infra/docker/storage-iceberg.Dockerfile \
	  --service-config storage-iceberg   deploy.startCommand  "/usr/local/bin/attest-storage-iceberg" \
	  --service-config detection-runtime build.builder        DOCKERFILE \
	  --service-config detection-runtime build.dockerfilePath infra/docker/detection-runtime.Dockerfile \
	  --service-config detection-runtime deploy.startCommand  "/usr/local/bin/attest-detection-runtime" \
	  --service-config arroyo-deployer   build.builder        DOCKERFILE \
	  --service-config arroyo-deployer   build.dockerfilePath infra/docker/arroyo-deployer.Dockerfile \
	  --service-config workbench         build.builder        NIXPACKS \
	  -m "configure builders and start commands for all application services"
	@echo "▶ Setting application env vars from infra/railway/*.json…"
	@./scripts/railway-set-env.sh
	@echo ""
	@echo "✔ Setup complete. Next: 'make railway-infra-deploy && make railway-app-start'"
	@echo "  (Infrastructure services — Kafka, RisingWave, ClickHouse, MinIO, Arroyo —"
	@echo "   must be added separately: see 'make railway-infra'.)"

railway-infra: ## Add infrastructure services (Kafka/KRaft, RisingWave, ClickHouse, MinIO, Arroyo) as Docker image services
	@echo "▶ Adding Kafka (Confluent KRaft — named 'redpanda' so app env vars stay unchanged)…"
	railway add --service redpanda --image confluentinc/cp-kafka:7.7.8 || true
	@echo "▶ Adding RisingWave…"
	railway add --service risingwave --image risingwavelabs/risingwave:latest || true
	@echo "▶ Adding ClickHouse…"
	railway add --service clickhouse --image clickhouse/clickhouse-server:latest || true
	@echo "▶ Adding MinIO…"
	railway add --service minio --image quay.io/minio/minio:latest || true
	@echo "▶ Adding Arroyo…"
	railway add --service arroyo --image ghcr.io/arroyosystems/arroyo:latest || true
	@echo ""
	@echo "✔ Infrastructure services created."
	@echo "  Run 'make railway-infra-config' to configure ports, start commands, env vars."
	@echo "  NOTE: Kafka requires a CLUSTER_ID — generate one with:"
	@echo "    python3 -c \"import base64,uuid; print(base64.urlsafe_b64encode(uuid.uuid4().bytes).decode().rstrip('='))\""

railway-infra-config: ## Configure infrastructure services env vars + start commands (non-interactive)
	@echo "▶ Setting env vars via railway variable set (non-interactive)…"
	@echo "  → redpanda (Kafka KRaft)"
	@if [ -z "$$CLUSTER_ID" ]; then \
	  CLUSTER_ID=$$(python3 -c "import base64,uuid; print(base64.urlsafe_b64encode(uuid.uuid4().bytes).decode().rstrip('='))"); \
	  echo "  ↳ No CLUSTER_ID set — generated: $$CLUSTER_ID"; \
	  echo "  ↳ WARNING: if Kafka already has data on disk, pass the original CLUSTER_ID instead."; \
	fi; \
	railway variable set -e production --service redpanda --skip-deploys \
	  CLUSTER_ID="$$CLUSTER_ID" \
	  KAFKA_NODE_ID=1 \
	  KAFKA_PROCESS_ROLES=broker,controller \
	  KAFKA_CONTROLLER_QUORUM_VOTERS="1@localhost:9093" \
	  KAFKA_LISTENERS="PLAINTEXT://:9092,CONTROLLER://:9093" \
	  KAFKA_ADVERTISED_LISTENERS="PLAINTEXT://redpanda.railway.internal:9092" \
	  KAFKA_CONTROLLER_LISTENER_NAMES=CONTROLLER \
	  KAFKA_LISTENER_SECURITY_PROTOCOL_MAP="CONTROLLER:PLAINTEXT,PLAINTEXT:PLAINTEXT" \
	  KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR=1 \
	  KAFKA_TRANSACTION_STATE_LOG_REPLICATION_FACTOR=1 \
	  KAFKA_TRANSACTION_STATE_LOG_MIN_ISR=1 \
	  KAFKA_AUTO_CREATE_TOPICS_ENABLE=true \
	  KAFKA_LOG_DIRS=/var/lib/kafka/data
	@echo "  → clickhouse"
	@railway variable set -e production --service clickhouse --skip-deploys \
	  CLICKHOUSE_USER=default \
	  CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1
	@echo "  → minio"
	@railway variable set -e production --service minio --skip-deploys \
	  MINIO_ROOT_USER=minioadmin \
	  MINIO_ROOT_PASSWORD=minioadmin
	@echo "  → arroyo"
	@railway variable set -e production --service arroyo --skip-deploys \
	  AWS_ACCESS_KEY_ID=minioadmin \
	  AWS_SECRET_ACCESS_KEY=minioadmin \
	  AWS_ENDPOINT_URL="http://minio.railway.internal:9000" \
	  AWS_ENDPOINT="http://minio.railway.internal:9000" \
	  AWS_REGION=us-east-1 \
	  AWS_ALLOW_HTTP=true \
	  KAFKA_BROKERS="redpanda.railway.internal:9092"
	@echo "✔ Infrastructure configured."
	@echo "  Run 'make railway-infra-deploy' to deploy/redeploy infra services with the new vars."

railway-domain: ## Generate a public domain for control-plane and workbench
	@echo "▶ Generating public domain for control-plane (needed for WebSocket)…"
	railway domain --service control-plane
	@echo "▶ Generating public domain for workbench…"
	railway domain --service workbench

railway-status: ## Show deployment status for all Railway services
	@echo ""
	@railway environment config
	@echo ""
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer orchestrator mcp-gateway calibration-sidecar redpanda risingwave clickhouse minio arroyo; do \
	  echo "── $$svc ──"; \
	  railway service status --service $$svc 2>&1 | grep -E "Status|status|ACTIVE|FAILED|CRASHED|SLEEPING|DEPLOYING|queued" | head -3 || true; \
	done

railway-logs-%: ## Stream runtime logs for a Railway service  (e.g. make railway-logs-collector)
	railway logs --service $*

railway-build-logs-%: ## Stream build logs for a Railway service  (e.g. make railway-build-logs-workbench)
	railway logs --service $* --build

railway-logs: ## Tail runtime logs for all application services (runs in parallel, Ctrl-C to stop)
	@echo "Tailing logs for all app services (Ctrl-C to stop)…"
	@railway logs --service collector &
	@railway logs --service control-plane &
	@railway logs --service storage-iceberg &
	@railway logs --service detection-runtime &
	@railway logs --service workbench &
	@railway logs --service arroyo-deployer &
	@wait

railway-stop: ## Stop all source-built app services (removes active deployments)
	@echo "▶ Stopping app services…"
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer orchestrator mcp-gateway calibration-sidecar; do \
	  echo "  → stopping $$svc"; \
	  railway down --service $$svc --yes 2>&1 | grep -v "^$$" || true; \
	done
	@echo "✔ App services stopped."
	@echo "  To stop infra (risingwave, redpanda, clickhouse, minio, arroyo) run: make railway-infra-stop"

railway-infra-stop: ## Stop all infrastructure services (risingwave, redpanda, clickhouse, minio, arroyo)
	@echo "▶ Stopping infrastructure services…"
	@for svc in risingwave redpanda clickhouse minio arroyo; do \
	  echo "  → stopping $$svc"; \
	  railway down --service $$svc --yes 2>&1 | grep -v "^$$" || true; \
	done
	@echo "✔ Infrastructure services stopped."
	@echo "  NOTE: The minio-volume will continue to be billed until deleted."

railway-infra-deploy: ## ⭐ Deploy infra services — handles first-deploy and redeployment automatically
	@echo "▶ Deploying infrastructure services (redpanda → risingwave → clickhouse → minio → arroyo)…"
	@echo "  Tries redeploy first; falls back to recreate only for stateless services."
	@for pair in \
	    "redpanda confluentinc/cp-kafka:7.7.8" \
	    "risingwave risingwavelabs/risingwave:latest" \
	    "clickhouse clickhouse/clickhouse-server:latest" \
	    "arroyo ghcr.io/arroyosystems/arroyo:latest"; do \
	  svc=$$(echo $$pair | cut -d' ' -f1); \
	  img=$$(echo $$pair | cut -d' ' -f2); \
	  echo "  → $$svc"; \
	  if railway service redeploy --service $$svc --yes 2>&1; then \
	    true; \
	  else \
	    echo "    ↳ no prior deploy — recreating service to trigger initial deployment…"; \
	    railway service delete --service $$svc --yes 2>/dev/null || true; \
	    railway add --service $$svc --image $$img --variables "_PLACEHOLDER=1"; \
	  fi; \
	done
	@echo "  → minio (volume-backed — never auto-deleted)"
	@if railway service redeploy --service minio --yes 2>&1; then \
	  true; \
	else \
	  echo "    ↳ minio has no prior deploy."; \
	  echo "    ↳ Go to the Railway dashboard → minio → Deploy, then set:"; \
	  echo "        Start command: server /data --console-address :9001"; \
	  echo "        Volume: attach minio-volume at /data"; \
	  echo "    ↳ Skipping auto-recreate to protect volume attachment."; \
	fi
	@echo ""
	@echo "✔ Infrastructure deploy triggered."
	@echo "  Wait ~60s for services to become healthy, then run 'make railway-app-start'."

railway-app-start: ## Deploy all application services — handles first-deploy and redeployment automatically
	@echo "▶ Deploying application services…"
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer orchestrator mcp-gateway calibration-sidecar; do \
	  echo "  → $$svc"; \
	  railway service redeploy --service $$svc --yes 2>&1 || \
	    railway up --service $$svc --detach --ci; \
	done
	@echo "✔ App services deploy triggered."

# `make prod pause` / `make prod resume` / `make prod down`
# Make treats the second word as another goal; these are no-ops so it does not fail.
PROD_ACTION := $(word 2,$(MAKECMDGOALS))
pause resume down:
	@:

prod: ## Railway demo: make prod pause | make prod resume | make prod down
	@if [ -z "$(PROD_ACTION)" ]; then \
	  echo "Usage: make prod pause | make prod resume | make prod down"; \
	  exit 2; \
	fi
	@./scripts/prod-railway.sh $(PROD_ACTION)

railway-full-deploy: ## ⭐ Full ordered deploy: infra first, wait 90s, then app services
	@echo "════════════════════════════════════════════════════════"
	@echo " Attest — full Railway deploy (infra → wait → apps)"
	@echo "════════════════════════════════════════════════════════"
	@echo ""
	@echo "Step 1/3 — Deploy infrastructure services…"
	@$(MAKE) railway-infra-deploy
	@echo ""
	@echo "Step 2/3 — Waiting 90s for infra to become healthy…"
	@echo "  (Kafka needs ~30s, RisingWave ~45s, ClickHouse ~20s)"
	@sleep 90
	@echo ""
	@echo "Step 3/3 — Deploy application services…"
	@$(MAKE) railway-app-start
	@echo ""
	@echo "✔ Full deploy complete. Run 'make railway-status' to verify."

load-gen-up: ## Start the load generator container (bench profile)
	docker compose --profile bench up -d load-gen

load-test: ## Quick smoke benchmark: 10k/sec for 30s against local load-gen
	curl -s -X POST http://localhost:9100/run \
	  -H "Content-Type: application/json" \
	  -d '{"rate":10000,"duration_secs":30,"scenario":"mixed","tenants":3,"seed_baselines":true}' \
	  | jq .

load-test-burst: ## Burst benchmark: 100k/sec for 60s (requires load-gen running)
	curl -s -X POST http://localhost:9100/run \
	  -H "Content-Type: application/json" \
	  -d '{"rate":100000,"duration_secs":60,"scenario":"mixed","tenants":5,"seed_baselines":true}' \
	  | jq .

load-status: ## Poll load generator status
	curl -s http://localhost:9100/status | jq .

load-stop: ## Stop the active load generator run
	curl -s -X POST http://localhost:9100/stop | jq .

load-cli-smoke: ## Run smoke benchmark directly via CLI (no HTTP, prints live stats)
	cargo run --release -p attest-load-gen -- --brokers localhost:19092 --rate 10000 --duration 30 --tenants 3 --seed-baselines

load-cli-burst: ## Run 100k/sec burst via CLI (prints live stats every 5s)
	cargo run --release -p attest-load-gen -- --brokers localhost:19092 --rate 100000 --duration 60 --scenario mixed --tenants 5 --seed-baselines

load-cli-attack: ## Run attack-only scenario via CLI to stress detection rules
	cargo run --release -p attest-load-gen -- --brokers localhost:19092 --rate 5000 --duration 60 --scenario attack --tenants 5 --seed-baselines
