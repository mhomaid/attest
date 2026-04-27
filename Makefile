.PHONY: help dev-up dev-down smoke seed-data fmt test lint railway-login railway-setup railway-deploy railway-redeploy railway-domain railway-status railway-logs train-classifier e2e-phase4a
.DEFAULT_GOAL := help

help: ## Show this help message
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n\nTargets:\n"} \
	  /^[a-zA-Z0-9_-]+:.*?##/ { printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

dev-up: ## Start all core services (Redpanda, RisingWave, ClickHouse, MinIO, Postgres)
	docker compose up -d

dev-down: ## Stop and remove all containers
	docker compose down

dev-up-platform: ## Start full platform stack (core + collector + control-plane + iceberg writer + detection runtime)
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
	@for svc in collector control-plane storage-iceberg detection-runtime workbench; do \
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
	  --service-config workbench         build.builder        NIXPACKS \
	  -m "configure builders and start commands for all application services"
	@echo "▶ Setting application env vars from infra/railway/*.json…"
	@./scripts/railway-set-env.sh
	@echo ""
	@echo "✔ Setup complete. Next: 'make railway-deploy'"
	@echo "  (Infrastructure services — Redpanda, RisingWave, ClickHouse, MinIO —"
	@echo "   must be added separately: see 'make railway-infra'.)"

railway-infra: ## Add infrastructure services (Redpanda, RisingWave, ClickHouse, MinIO) as Docker image services
	@echo "▶ Adding Redpanda…"
	railway add --service redpanda --image redpandadata/redpanda:v26.1.6 || true
	@echo "▶ Adding RisingWave…"
	railway add --service risingwave --image risingwavelabs/risingwave:latest || true
	@echo "▶ Adding ClickHouse…"
	railway add --service clickhouse --image clickhouse/clickhouse-server:latest || true
	@echo "▶ Adding MinIO…"
	railway add --service minio --image minio/minio:latest || true
	@echo ""
	@echo "✔ Infrastructure services created."
	@echo "  Run 'make railway-infra-config' to configure ports, start commands, env vars."

railway-infra-config: ## Configure infrastructure services (start commands, env vars, ports)
	@railway environment edit \
	  --service-config redpanda   deploy.startCommand  "redpanda start --mode dev-container --smp 1 --memory 1G --kafka-addr PLAINTEXT://0.0.0.0:9092 --advertise-kafka-addr PLAINTEXT://redpanda.railway.internal:9092" \
	  --service-config risingwave deploy.startCommand  "playground" \
	  --service-config clickhouse variables.CLICKHOUSE_USER.value default \
	  --service-config clickhouse variables.CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT.value 1 \
	  --service-config minio      deploy.startCommand  "server /data --console-address :9001" \
	  --service-config minio      variables.MINIO_ROOT_USER.value minioadmin \
	  --service-config minio      variables.MINIO_ROOT_PASSWORD.value minioadmin \
	  -m "configure infrastructure services"
	@echo "✔ Infrastructure configured."

railway-deploy: ## Upload local source and deploy all application services to Railway (first deploy)
	@for svc in collector control-plane storage-iceberg detection-runtime workbench; do \
	  echo "▶ Deploying $$svc …"; \
	  railway up --service $$svc --detach --ci; \
	done

railway-redeploy: ## Trigger redeploy of the latest deployment for all services (after first deploy)
	@for svc in collector control-plane storage-iceberg detection-runtime workbench; do \
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
	@for svc in collector control-plane storage-iceberg detection-runtime workbench redpanda risingwave clickhouse minio; do \
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
	@wait
