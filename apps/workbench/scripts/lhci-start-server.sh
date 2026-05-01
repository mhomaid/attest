#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export DATABASE_URL="${DATABASE_URL:-postgres://attest:attest@127.0.0.1:5432/attest}"
export ALLOW_AUTH_SEED="${ALLOW_AUTH_SEED:-1}"
export AUTH_SEED_SECRET="${AUTH_SEED_SECRET:-ci-auth-seed-secret}"
export BETTER_AUTH_SECRET="${BETTER_AUTH_SECRET:-local-dev-better-auth-secret-min-32-chars!!}"
exec bun run dev -- -p 3000
