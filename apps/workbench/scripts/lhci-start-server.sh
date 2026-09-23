#!/usr/bin/env bash
# Production standalone server for Lighthouse.
# Prints LHCI_SERVER_READY only after 127.0.0.1:3000 accepts TCP — do not use
# a generic "Ready" pattern; Next can log that before the socket is open, and
# GitHub runners resolve localhost to IPv6 while Next binds IPv4.
set -euo pipefail
cd "$(dirname "$0")/.."

export DATABASE_URL="${DATABASE_URL:-postgres://attest:attest@127.0.0.1:5432/attest}"
if [ -z "${BETTER_AUTH_SECRET:-}" ] || [ "$BETTER_AUTH_SECRET" = "local-dev-better-auth-secret-min-32-chars!!" ]; then
  export BETTER_AUTH_SECRET="lhci-better-auth-secret-min-32-chars!!"
fi
export NODE_ENV=production
export PORT="${PORT:-3000}"
export HOSTNAME="127.0.0.1"
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

node server.js &
server_pid=$!
cleanup() { kill "$server_pid" 2>/dev/null || true; }
trap cleanup EXIT INT TERM

for _ in $(seq 1 60); do
  if curl -sf --max-time 1 "http://127.0.0.1:${PORT}" >/dev/null; then
    echo "LHCI_SERVER_READY"
    wait "$server_pid"
    exit $?
  fi
  if ! kill -0 "$server_pid" 2>/dev/null; then
    echo "error: workbench server exited before opening port ${PORT}" >&2
    wait "$server_pid" || true
    exit 1
  fi
  sleep 1
done

echo "error: workbench server did not accept connections on 127.0.0.1:${PORT} within 60s" >&2
exit 1
