#!/bin/sh
set -eu
: "${DATABASE_URL:?DATABASE_URL is required}"
echo "Running Attest SQL migrations against Postgres…"
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f /sql/0001_auth.sql
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f /sql/0002_seed_dev_analyst.sql
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f /sql/0003_seed_dev_team.sql
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f /sql/0004_workbench_cases.sql
echo "Migrations complete."
