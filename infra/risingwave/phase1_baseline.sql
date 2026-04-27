-- Phase 1 — Streaming Substrate: RisingWave DDL
-- Applied by attest-control-plane on first boot (idempotent via IF NOT EXISTS).
-- Flat JSON schema matches FlatEvent produced by attest-collector.

-- ── 1. Kafka source table (Redpanda `cloudtrail` topic) ──────────────────

CREATE TABLE IF NOT EXISTS cloudtrail_events (
  event_id          VARCHAR,
  class_uid         VARCHAR,
  time              TIMESTAMPTZ,
  tenant_id         VARCHAR,
  actor_user_name   VARCHAR,
  actor_user_uid    VARCHAR,
  cloud_region      VARCHAR,
  cloud_account_uid VARCHAR,
  severity          VARCHAR,
  auth_status       VARCHAR,
  api_operation     VARCHAR,
  api_service       VARCHAR,
  raw               JSONB
) WITH (
  connector                     = 'kafka',
  topic                         = 'cloudtrail',
  properties.bootstrap.server   = 'redpanda:9092',
  scan.startup.mode             = 'earliest'
) FORMAT PLAIN ENCODE JSON;

-- ── 2. Recent events index (last 1 hour) ────────────────────────────────

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

-- ── 3. Per-entity baseline (last 30 days) ───────────────────────────────

CREATE MATERIALIZED VIEW IF NOT EXISTS entity_baselines AS
SELECT
  tenant_id,
  actor_user_name,
  ARRAY_AGG(DISTINCT cloud_region) FILTER (WHERE cloud_region IS NOT NULL) AS regions_seen_30d,
  COUNT(*)                                                                  AS event_count_30d,
  MAX(time)                                                                 AS last_seen
FROM cloudtrail_events
WHERE time >= NOW() - INTERVAL '30 days'
GROUP BY tenant_id, actor_user_name;
