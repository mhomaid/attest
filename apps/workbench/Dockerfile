# syntax=docker/dockerfile:1
# ─────────────────────────────────────────────────────────────────────────────
# Stage 1: deps
#   Install production + dev dependencies with bun.
# ─────────────────────────────────────────────────────────────────────────────
FROM oven/bun:1 AS deps
WORKDIR /app

# Copy manifests first for layer caching.
COPY apps/workbench/package.json ./apps/workbench/package.json
COPY bun.lock ./bun.lock
COPY package.json ./package.json

# Install from workspace root so bun resolves the workspace protocol.
RUN bun install --frozen-lockfile

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2: builder
#   Build the Next.js standalone bundle.
# ─────────────────────────────────────────────────────────────────────────────
FROM oven/bun:1 AS builder
WORKDIR /app

COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/apps/workbench/node_modules ./apps/workbench/node_modules

# Copy the full workbench source.
COPY apps/workbench ./apps/workbench
# Copy workspace root package.json (needed by bun workspace resolution).
COPY package.json ./package.json
COPY bun.lock ./bun.lock

WORKDIR /app/apps/workbench

# Sentry source maps require these at build time; pass as build args or leave
# empty — the build succeeds either way (silenced when CI is unset).
ARG SENTRY_AUTH_TOKEN
ARG SENTRY_ORG
ARG SENTRY_PROJECT
ARG NEXT_PUBLIC_POSTHOG_KEY
ARG NEXT_PUBLIC_POSTHOG_HOST
ARG NEXT_PUBLIC_SENTRY_DSN
ARG NEXT_PUBLIC_APP_URL
ARG NEXT_PUBLIC_CP_URL
ARG NEXT_PUBLIC_CP_WS_URL
ARG NEXT_PUBLIC_WS_GATEWAY_URL

ENV NEXT_TELEMETRY_DISABLED=1
ENV NODE_ENV=production

RUN bun run build

# ─────────────────────────────────────────────────────────────────────────────
# Stage 3: runner
#   Minimal image — only the standalone output + public assets.
# ─────────────────────────────────────────────────────────────────────────────
FROM node:22-alpine AS runner
WORKDIR /app

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
# Railway injects PORT automatically.
ENV PORT=3000

RUN addgroup --system --gid 1001 nodejs \
 && adduser  --system --uid 1001 nextjs

# Copy the standalone server bundle.
COPY --from=builder --chown=nextjs:nodejs \
  /app/apps/workbench/.next/standalone ./

# Copy static assets (CSS, JS chunks, fonts).
COPY --from=builder --chown=nextjs:nodejs \
  /app/apps/workbench/.next/static ./apps/workbench/.next/static

# Copy the public folder (favicons, images, robots.txt, etc.).
COPY --from=builder --chown=nextjs:nodejs \
  /app/apps/workbench/public ./apps/workbench/public

USER nextjs

EXPOSE 3000

# next start --standalone writes server.js at the workspace-relative path.
CMD ["node", "apps/workbench/server.js"]
