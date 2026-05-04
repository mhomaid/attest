# syntax=docker/dockerfile:1
# Build context: apps/workbench/ (railway up run from this directory)
# ─────────────────────────────────────────────────────────────────────────────
# Stage 1: deps
# ─────────────────────────────────────────────────────────────────────────────
FROM oven/bun:1 AS deps
WORKDIR /app

COPY package.json bun.lockb* ./
# bun.lock (text lockfile, Bun >=1.1.x) or bun.lockb (binary)
COPY bun.lock* ./

RUN bun install --frozen-lockfile || bun install

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2: builder
# ─────────────────────────────────────────────────────────────────────────────
FROM oven/bun:1 AS builder
WORKDIR /app

COPY --from=deps /app/node_modules ./node_modules
COPY . .

ARG SENTRY_AUTH_TOKEN
ARG SENTRY_ORG
ARG SENTRY_PROJECT
ARG NEXT_PUBLIC_POSTHOG_KEY
ARG NEXT_PUBLIC_POSTHOG_HOST
ARG NEXT_PUBLIC_SENTRY_DSN
ARG NEXT_PUBLIC_APP_URL
ARG NEXT_PUBLIC_CP_URL=http://localhost:8080
ARG NEXT_PUBLIC_CP_WS_URL=ws://localhost:8080
ARG NEXT_PUBLIC_WS_GATEWAY_URL=ws://localhost:4500

ENV NEXT_TELEMETRY_DISABLED=1
ENV NODE_ENV=production

RUN bun run build

# ─────────────────────────────────────────────────────────────────────────────
# Stage 3: runner — minimal image, standalone output only
# ─────────────────────────────────────────────────────────────────────────────
FROM node:22-alpine AS runner
WORKDIR /app

ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1
ENV PORT=3000
ENV HOSTNAME=0.0.0.0

RUN addgroup --system --gid 1001 nodejs \
 && adduser  --system --uid 1001 nextjs

COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static     ./.next/static
COPY --from=builder --chown=nextjs:nodejs /app/public           ./public

USER nextjs
EXPOSE 3000

CMD ["node", "server.js"]
