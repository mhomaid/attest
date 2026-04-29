.PHONY: help dev-up dev-down smoke seed-data fmt test lint railway-login railway-setup railway-deploy railway-redeploy railway-domain railway-status railway-logs railway-stop railway-start railway-infra-stop railway-infra-start railway-infra-first-deploy railway-app-start railway-full-deploy railway-full-redeploy train-classifier e2e-phase4a arroyo-ui arroyo-deploy e2e-arroyo railway-logs-collector railway-logs-control-plane railway-logs-workbench railway-logs-arroyo
.DEFAULT_GOAL := help

help: ## Show this help message
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n\nTargets:\n"} \
	  /^[a-zA-Z0-9_-]+:.*?##/ { printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

dev-up: ## Start all core services (Redpanda, RisingWave, ClickHouse, MinIO, Postgres)
	docker compose up -d

dev-down: ## Stop and remove all containers
	docker compose down

dev-up-platform: ## Start full platform stack (core + collector + control-plane + iceberg writer + detection runtime + arroyo)
	docker compose up -d redpanda postgres minio clickhouse risingwave
	docker compose run --rm minio-init
	docker compose --profile platform up -d --build

e2e-phase1: ## Run Phase 1 E2E test — event appears in RisingWave within 5s (requires ATTEST_E2E=1 + platform)
	ATTEST_E2E=1 cargo test --test phase1_streaming -- --nocapture

e2e-phase2: ## Run Phase 2 E2E test — 10K events land in MinIO Parquet + ClickHouse query (requires ATTEST_E2E=1 + platform)
	ATTEST_E2E=1 cargo test --test phase2_iceberg -- --nocapture

e2e-phase3: ## Run Phase 3 E2E test — HELIQL detections fire alerts on stream (requires ATTEST_E2E=1 + platform)
	ATTEST_E2E=1 cargo test --test phase3_detection -- --nocapture

train-classifier: ## Train XGBoost classifier + novelty detector + calibration (outputs to ml/triager/artifacts/)
	cd ml && uv run python triager/train.py
	cd ml && uv run python triager/novelty.py
	cd ml && uv run python triager/calibrate.py --train

e2e-phase4a: ## Run Phase 4a E2E tests — starts services, runs tests, cleans up
	@echo "==> Starting calibration sidecar (port 5001)..."
	cd ml && CALIBRATION_PORT=5001 uv run python triager/calibrate.py --serve > /tmp/attest-calibration.log 2>&1 &
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

arroyo-ui: ## Open Arroyo web UI in the browser (http://localhost:5115)
	open http://localhost:5115

e2e-arroyo: ## Run Arroyo E2E tests — health, pipeline deploy, ETL Parquet, CEP alert (requires dev-up-platform)
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
	uv run pytest ml/

seed-data: ## Download Tier 1 datasets into MinIO and Postgres (see 08_Datasets_and_ML.md)
	@echo "TODO: implement seed-data (08_Datasets_and_ML.md §2.1)"

fmt: ## Format all Rust code
	cargo fmt --all

test: ## Run all tests: Rust workspace + JS + Python
	cargo test --workspace
	bun run test
	uv run pytest ml/

lint: ## Lint all code: clippy + bun lint
	cargo clippy --workspace --all-targets -- -D warnings
	bun run lint

# ── Railway deployment ────────────────────────────────────────────────────────

railway-login: ## Log in to Railway CLI
	railway login

railway-setup: ## Create all services AND configure each one's builder (Dockerfile path or Nixpacks) — fully CLI driven
	@echo "▶ Creating services (skipping if they already exist)…"
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer; do \
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
	@echo "✔ Setup complete. Next: 'make railway-deploy'"
	@echo "  (Infrastructure services — Kafka, RisingWave, ClickHouse, MinIO, Arroyo —"
	@echo "   must be added separately: see 'make railway-infra'.)"

railway-infra: ## Add infrastructure services (Kafka/KRaft, RisingWave, ClickHouse, MinIO, Arroyo) as Docker image services
	@echo "▶ Adding Kafka (Confluent KRaft — named 'redpanda' so app env vars stay unchanged)…"
	railway add --service redpanda --image confluentinc/cp-kafka:7.9.0 || true
	@echo "▶ Adding RisingWave…"
	railway add --service risingwave --image risingwavelabs/risingwave:latest || true
	@echo "▶ Adding ClickHouse…"
	railway add --service clickhouse --image clickhouse/clickhouse-server:latest || true
	@echo "▶ Adding MinIO…"
	railway add --service minio --image minio/minio:latest || true
	@echo "▶ Adding Arroyo…"
	railway add --service arroyo --image ghcr.io/arroyosystems/arroyo:latest || true
	@echo ""
	@echo "✔ Infrastructure services created."
	@echo "  Run 'make railway-infra-config' to configure ports, start commands, env vars."
	@echo "  NOTE: Kafka requires a CLUSTER_ID — generate one with:"
	@echo "    python3 -c \"import base64,uuid; print(base64.urlsafe_b64encode(uuid.uuid4().bytes).decode().rstrip('='))\""

railway-infra-config: ## Configure infrastructure services (start commands, env vars, ports)
	@echo "▶ Configuring Kafka (Confluent KRaft) start command and env vars…"
	@echo "  Requires CLUSTER_ID env var — generate with:"
	@echo "    export CLUSTER_ID=\$$(python3 -c \"import base64,uuid; print(base64.urlsafe_b64encode(uuid.uuid4().bytes).decode().rstrip('='))\")"
	@[ -n "$$CLUSTER_ID" ] || (echo "ERROR: CLUSTER_ID not set"; exit 1)
	@railway environment edit \
	  --service-config redpanda   deploy.startCommand  "/etc/confluent/docker/run" \
	  --service-config redpanda   variables.CLUSTER_ID.value "$$CLUSTER_ID" \
	  --service-config redpanda   variables.KAFKA_NODE_ID.value "1" \
	  --service-config redpanda   variables.KAFKA_PROCESS_ROLES.value "broker,controller" \
	  --service-config redpanda   variables.KAFKA_CONTROLLER_QUORUM_VOTERS.value "1@localhost:9093" \
	  --service-config redpanda   variables.KAFKA_LISTENERS.value "PLAINTEXT://:9092,CONTROLLER://:9093" \
	  --service-config redpanda   variables.KAFKA_ADVERTISED_LISTENERS.value "PLAINTEXT://redpanda.railway.internal:9092" \
	  --service-config redpanda   variables.KAFKA_CONTROLLER_LISTENER_NAMES.value "CONTROLLER" \
	  --service-config redpanda   variables.KAFKA_LISTENER_SECURITY_PROTOCOL_MAP.value "CONTROLLER:PLAINTEXT,PLAINTEXT:PLAINTEXT" \
	  --service-config redpanda   variables.KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR.value "1" \
	  --service-config redpanda   variables.KAFKA_TRANSACTION_STATE_LOG_REPLICATION_FACTOR.value "1" \
	  --service-config redpanda   variables.KAFKA_TRANSACTION_STATE_LOG_MIN_ISR.value "1" \
	  --service-config redpanda   variables.KAFKA_AUTO_CREATE_TOPICS_ENABLE.value "true" \
	  --service-config redpanda   variables.KAFKA_LOG_DIRS.value "/var/lib/kafka/data" \
	  --service-config risingwave deploy.startCommand  "playground" \
	  --service-config clickhouse variables.CLICKHOUSE_USER.value default \
	  --service-config clickhouse variables.CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT.value 1 \
	  --service-config minio      deploy.startCommand  "minio server /data --console-address :9001" \
	  --service-config minio      variables.MINIO_ROOT_USER.value minioadmin \
	  --service-config minio      variables.MINIO_ROOT_PASSWORD.value minioadmin \
	  --service-config arroyo     variables.AWS_ACCESS_KEY_ID.value minioadmin \
	  --service-config arroyo     variables.AWS_SECRET_ACCESS_KEY.value minioadmin \
	  --service-config arroyo     variables.AWS_ENDPOINT_URL.value "http://minio.railway.internal:9000" \
	  --service-config arroyo     variables.AWS_ENDPOINT.value "http://minio.railway.internal:9000" \
	  --service-config arroyo     variables.AWS_REGION.value us-east-1 \
	  --service-config arroyo     variables.AWS_ALLOW_HTTP.value "true" \
	  --service-config arroyo     variables.KAFKA_BROKERS.value "redpanda.railway.internal:9092" \
	  -m "configure infrastructure services (Confluent KRaft + MinIO + Arroyo)"
	@echo "✔ Infrastructure configured."
	@echo "  Next: attach a Railway volume to minio at /data, then run 'make railway-setup'."

railway-deploy: ## Upload local source and deploy all application services to Railway (first deploy)
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer; do \
	  echo "▶ Deploying $$svc …"; \
	  railway up --service $$svc --detach --ci; \
	done

railway-redeploy: ## Trigger redeploy of the latest deployment for all services (after first deploy)
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer; do \
	  echo "▶ Redeploying $$svc …"; \
	  railway service redeploy --service $$svc --yes; \
	done

railway-domain: ## Generate a public domain for control-plane and workbench
	@echo "▶ Generating public domain for control-plane (needed for WebSocket)…"
	railway domain --service control-plane
	@echo "▶ Generating public domain for workbench…"
	railway domain --service workbench

railway-status: ## Show deployment status for all Railway services
	@echo ""
	@railway environment config
	@echo ""
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer redpanda risingwave clickhouse minio arroyo; do \
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
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer; do \
	  echo "  → stopping $$svc"; \
	  railway down --service $$svc --yes 2>&1 | grep -v "^$$" || true; \
	done
	@echo "✔ App services stopped."
	@echo "  To stop infra (risingwave, redpanda, clickhouse, minio, arroyo) run: make railway-infra-stop"

railway-start: ## Redeploy all source-built app services (assumes infra is already running)
	@echo "▶ Starting app services…"
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer; do \
	  echo "  → starting $$svc"; \
	  railway redeploy --service $$svc --yes; \
	done
	@echo "✔ App services started. Run 'make railway-status' to verify."

railway-infra-stop: ## Stop all infrastructure services (risingwave, redpanda, clickhouse, minio, arroyo)
	@echo "▶ Stopping infrastructure services…"
	@for svc in risingwave redpanda clickhouse minio arroyo; do \
	  echo "  → stopping $$svc"; \
	  railway down --service $$svc --yes 2>&1 | grep -v "^$$" || true; \
	done
	@echo "✔ Infrastructure services stopped."
	@echo "  NOTE: The minio-volume will continue to be billed until deleted."

railway-infra-first-deploy: ## First-time deploy of ALL infra image services (deletes stale definitions, re-adds with image → auto-deploys)
	@echo "▶ First-time infrastructure deploy — deleting stale service definitions and re-adding with images…"
	@echo "  (Safe to run even if services don't exist yet. All data in Railway volumes is preserved.)"
	@echo ""
	@echo "  → redpanda (Confluent KRaft Kafka)"
	@railway service delete --service redpanda --yes 2>/dev/null || true
	@railway add --service redpanda --image confluentinc/cp-kafka:7.9.0
	@echo ""
	@echo "  → risingwave"
	@railway service delete --service risingwave --yes 2>/dev/null || true
	@railway add --service risingwave --image risingwavelabs/risingwave:latest
	@echo ""
	@echo "  → clickhouse"
	@railway service delete --service clickhouse --yes 2>/dev/null || true
	@railway add --service clickhouse --image clickhouse/clickhouse-server:latest
	@echo ""
	@echo "  → minio"
	@railway service delete --service minio --yes 2>/dev/null || true
	@railway add --service minio --image minio/minio:latest
	@echo ""
	@echo "  → arroyo"
	@railway service delete --service arroyo --yes 2>/dev/null || true
	@railway add --service arroyo --image ghcr.io/arroyosystems/arroyo:latest
	@echo ""
	@echo "✔ All infra services created and auto-deploying."
	@echo "  Now run (requires CLUSTER_ID for Kafka):"
	@echo "    export CLUSTER_ID=\$$(python3 -c \"import base64,uuid; print(base64.urlsafe_b64encode(uuid.uuid4().bytes).decode().rstrip('='))\")"
	@echo "    make railway-infra-config"
	@echo "  Then wait ~60s and run:"
	@echo "    make railway-app-start"

railway-infra-start: ## (Re)deploy all infrastructure services — works for both first-deploy and redeploy
	@echo "▶ Deploying infrastructure services (redpanda → risingwave → clickhouse → minio → arroyo)…"
	@echo "  This triggers a new deployment for each; safe to run if services are already running."
	@for svc in redpanda risingwave clickhouse minio arroyo; do \
	  echo "  → $$svc"; \
	  railway service redeploy --service $$svc --yes 2>&1 || \
	    echo "    ⚠ $$svc: no existing deployment — open Railway dashboard and click Deploy once, then re-run."; \
	done
	@echo ""
	@echo "✔ Infrastructure deploy triggered."
	@echo "  Wait ~60s for services to become healthy, then run 'make railway-app-start'."

railway-app-start: ## Deploy all application services (assumes infra is already healthy)
	@echo "▶ Deploying application services…"
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer; do \
	  echo "  → $$svc"; \
	  railway service redeploy --service $$svc --yes 2>&1 || \
	    railway up --service $$svc --detach --ci; \
	done
	@echo "✔ App services deploy triggered."

railway-full-deploy: ## ⭐ Full ordered deploy: infra first, wait 90s, then app services
	@echo "════════════════════════════════════════════════════════"
	@echo " Attest — full Railway deploy (infra → wait → apps)"
	@echo "════════════════════════════════════════════════════════"
	@echo ""
	@echo "Step 1/3 — Deploy infrastructure services…"
	@for svc in redpanda risingwave clickhouse minio arroyo; do \
	  echo "  → $$svc"; \
	  railway service redeploy --service $$svc --yes 2>&1 || \
	    echo "    ⚠ $$svc has no prior deployment — open the Railway dashboard and click Deploy, then re-run."; \
	done
	@echo ""
	@echo "Step 2/3 — Waiting 90s for infra to become healthy…"
	@echo "  (Kafka needs ~30s, RisingWave ~45s, ClickHouse ~20s)"
	@sleep 90
	@echo ""
	@echo "Step 3/3 — Deploy application services…"
	@for svc in collector control-plane storage-iceberg detection-runtime workbench arroyo-deployer; do \
	  echo "  → $$svc"; \
	  railway service redeploy --service $$svc --yes 2>&1 || \
	    railway up --service $$svc --detach --ci; \
	done
	@echo ""
	@echo "✔ Full deploy complete. Run 'make railway-status' to verify."

railway-full-redeploy: ## Full ordered redeploy (same as full-deploy, alias for clarity)
	@$(MAKE) railway-full-deploy
