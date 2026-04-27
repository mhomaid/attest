-- Phase 1 — Streaming Substrate: RisingWave DDL
-- Applied by attest-control-plane on first boot (idempotent via IF NOT EXISTS).
--
-- RisingWave speaks the Postgres wire protocol on port 4566.
-- Connect: psql -h localhost -p 4566 -d dev

-- ── 1. Kafka source (Redpanda cloudtrail topic) ──────────────────────────

CREATE TABLE IF NOT EXISTS cloudtrail_events (
  event_id       VARCHAR,
  class_uid      VARCHAR,
  time           TIMESTAMPTZ,
  tenant_id      VARCHAR,
  actor_user_name VARCHAR,
  actor_user_uid  VARCHAR,
  cloud_region    VARCHAR,
  cloud_account_uid VARCHAR,
  severity        VARCHAR,
  -- Authentication-specific
  auth_status     VARCHAR,
  -- CloudActivity-specific
  api_operation   VARCHAR,
  api_service     VARCHAR,
  -- Preserve full payload for attribution
  raw             JSONB
) WITH (
  connector      = 'kafka',
  topic          = 'cloudtrail',
  properties.bootstrap.server = 'redpanda:9092',
  scan.startup.mode = 'earliest'
) FORMAT PLAIN ENCODE JSON;

-- ── 2. Per-entity baseline materialized view ─────────────────────────────
-- Tracks which regions each user has been seen in over the last 30 days.
-- Used by /v1/baselines/user/:name in the control-plane API.

CREATE MATERIALIZED VIEW IF NOT EXISTS entity_baselines AS
SELECT
  tenant_id,
  actor_user_name,
  ARRAY_AGG(DISTINCT cloud_region) FILTER (WHERE cloud_region IS NOT NULL) AS regions_seen_30d,
  COUNT(*)                         AS event_count_30d,
  MAX(time)                        AS last_seen
FROM cloudtrail_events
WHERE time >= NOW() - INTERVAL '30 days'
GROUP BY tenant_id, actor_user_name;

-- ── 3. Recent events indexed view (for /v1/events/recent) ────────────────
-- Lightweight lookup: find an event by its event_id.

CREATE MATERIALIZED VIEW IF NOT EXISTS recent_events AS
SELECT
  event_id,
  class_uid,
  time,
  tenant_id,
  actor_user_name,
  actor_user_uid,
  cloud_region,
  cloud_account_uid,
  severity,
  auth_status,
  api_operation,
  api_service,
  raw
FROM cloudtrail_events
WHERE time >= NOW() - INTERVAL '1 hour';
