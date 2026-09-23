#!/usr/bin/env bash
# Production server for Lighthouse. `next dev` scores ~0.75 on GitHub runners
# (compile-on-request); the 0.9 budget is for the standalone build we ship.
set -euo pipefail
cd "$(dirname "$0")/.."

export DATABASE_URL="${DATABASE_URL:-postgres://attest:attest@127.0.0.1:5432/attest}"
# Must not be the public repo secret — auth.ts refuses that when NODE_ENV=production.
if [ -z "${BETTER_AUTH_SECRET:-}" ] || [ "$BETTER_AUTH_SECRET" = "local-dev-better-auth-secret-min-32-chars!!" ]; then
  export BETTER_AUTH_SECRET="lhci-better-auth-secret-min-32-chars!!"
fi
export NODE_ENV=production
export PORT="${PORT:-3000}"
export HOSTNAME="${HOSTNAME:-0.0.0.0}"
export NEXT_TELEMETRY_DISABLED=1

if [ ! -f .next/standalone/server.js ] && [ ! -f .next/standalone/apps/workbench/server.js ]; then
  bun run build
fi

if [ -f .next/standalone/apps/workbench/server.js ]; then
  root=".next/standalone"
  mkdir -p "$root/apps/workbench/.next"
  rm -rf "$root/apps/workbench/.next/static" "$root/apps/workbench/public"
  cp -R .next/static "$root/apps/workbench/.next/static"
  cp -R public "$root/apps/workbench/public"
  cd "$root/apps/workbench"
elif [ -f .next/standalone/server.js ]; then
  mkdir -p .next/standalone/.next
  rm -rf .next/standalone/.next/static .next/standalone/public
  cp -R .next/static .next/standalone/.next/static
  cp -R public .next/standalone/public
  cd .next/standalone
else
  echo "error: standalone server.js missing after next build" >&2
  exit 1
fi

exec node server.js
