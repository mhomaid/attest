.PHONY: help dev-up dev-down smoke seed-data fmt test lint
.DEFAULT_GOAL := help

help: ## Show this help message
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n\nTargets:\n"} \
	  /^[a-zA-Z_-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

dev-up: ## Start all core services (Redpanda, RisingWave, ClickHouse, MinIO, Postgres)
	docker compose up -d

dev-down: ## Stop and remove all containers
	docker compose down

dev-up-platform: ## Start core services + collector + control-plane (Phase 1 full stack)
	docker compose up -d redpanda postgres minio clickhouse risingwave
	docker compose --profile platform up -d

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
