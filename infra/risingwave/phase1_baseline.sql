-- Phase 1 — Streaming Substrate: RisingWave DDL
-- Applied by attest-control-plane on first boot (idempotent via IF NOT EXISTS).
-- Flat JSON schema matches FlatEvent produced by attest-collector.
-- All det_* views expose: detection_id, event_id, actor_user_name,
--   cloud_region, severity, fired_at  (columns consumed by fetch_all_fired).

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

-- ── 4. Detection views (det_*) ───────────────────────────────────────────
-- Each view fires a row per matching event; the control-plane polls them
-- every 2 s and broadcasts new rows over the WebSocket.

CREATE MATERIALIZED VIEW IF NOT EXISTS det_aws_root_account_use AS
SELECT
  CONCAT('det_aws_root_account_use:', event_id) AS detection_id,
  event_id,
  COALESCE(actor_user_name, 'root')              AS actor_user_name,
  COALESCE(cloud_region, 'global')               AS cloud_region,
  'critical'                                     AS severity,
  time::VARCHAR                                  AS fired_at
FROM cloudtrail_events
WHERE actor_user_name = 'root'
   OR actor_user_uid LIKE '%:root';

CREATE MATERIALIZED VIEW IF NOT EXISTS det_aws_cloudtrail_logging_disabled AS
SELECT
  CONCAT('det_aws_cloudtrail_logging_disabled:', event_id) AS detection_id,
  event_id,
  COALESCE(actor_user_name, '')  AS actor_user_name,
  COALESCE(cloud_region, '')     AS cloud_region,
  'high'                         AS severity,
  time::VARCHAR                  AS fired_at
FROM cloudtrail_events
WHERE api_operation IN ('StopLogging', 'DeleteTrail', 'UpdateTrail');

CREATE MATERIALIZED VIEW IF NOT EXISTS det_aws_s3_bucket_policy_made_public AS
SELECT
  CONCAT('det_aws_s3_bucket_policy_made_public:', event_id) AS detection_id,
  event_id,
  COALESCE(actor_user_name, '') AS actor_user_name,
  COALESCE(cloud_region, '')    AS cloud_region,
  'high'                        AS severity,
  time::VARCHAR                 AS fired_at
FROM cloudtrail_events
WHERE api_operation IN ('PutBucketAcl', 'PutBucketPolicy')
  AND api_service = 's3.amazonaws.com'
  AND (raw->>'requestParameters')::TEXT ILIKE '%AllUsers%';

CREATE MATERIALIZED VIEW IF NOT EXISTS det_aws_console_login_from_anomalous_geolocation AS
SELECT
  CONCAT('det_aws_console_login_from_anomalous_geolocation:', event_id) AS detection_id,
  event_id,
  COALESCE(actor_user_name, '') AS actor_user_name,
  COALESCE(cloud_region, '')    AS cloud_region,
  'medium'                      AS severity,
  time::VARCHAR                 AS fired_at
FROM cloudtrail_events
WHERE api_operation = 'ConsoleLogin'
  AND auth_status   = 'success'
  AND (raw->>'sourceIPAddress') IS NOT NULL;

CREATE MATERIALIZED VIEW IF NOT EXISTS det_aws_new_iam_user_then_access_keys_sequence AS
SELECT
  CONCAT('det_aws_new_iam_user_then_access_keys_sequence:', event_id) AS detection_id,
  event_id,
  COALESCE(actor_user_name, '') AS actor_user_name,
  COALESCE(cloud_region, '')    AS cloud_region,
  'high'                        AS severity,
  time::VARCHAR                 AS fired_at
FROM cloudtrail_events
WHERE api_operation IN ('CreateUser', 'CreateAccessKey')
  AND api_service = 'iam.amazonaws.com';

CREATE MATERIALIZED VIEW IF NOT EXISTS det_aws_iam_user_excessive_privilege AS
SELECT
  CONCAT('det_aws_iam_user_excessive_privilege:', event_id) AS detection_id,
  event_id,
  COALESCE(actor_user_name, '') AS actor_user_name,
  COALESCE(cloud_region, '')    AS cloud_region,
  'high'                        AS severity,
  time::VARCHAR                 AS fired_at
FROM cloudtrail_events
WHERE api_operation IN (
  'PutUserPolicy', 'AttachUserPolicy', 'PutRolePolicy', 'AttachRolePolicy'
)
  AND api_service = 'iam.amazonaws.com'
  AND (raw->>'requestParameters')::TEXT ILIKE '%AdministratorAccess%';

CREATE MATERIALIZED VIEW IF NOT EXISTS det_okta_mfa_bypass_attempt AS
SELECT
  CONCAT('det_okta_mfa_bypass_attempt:', event_id) AS detection_id,
  event_id,
  COALESCE(actor_user_name, '') AS actor_user_name,
  COALESCE(cloud_region, '')    AS cloud_region,
  'high'                        AS severity,
  time::VARCHAR                 AS fired_at
FROM cloudtrail_events
WHERE class_uid   = 'okta'
  AND api_operation IN ('user.mfa.factor.deactivate', 'user.mfa.factor.reset_all',
                        'policy.rule.update', 'policy.rule.delete');

CREATE MATERIALIZED VIEW IF NOT EXISTS det_okta_brute_force_authentication AS
SELECT
  CONCAT('det_okta_brute_force_authentication:', event_id) AS detection_id,
  event_id,
  COALESCE(actor_user_name, '') AS actor_user_name,
  COALESCE(cloud_region, '')    AS cloud_region,
  'medium'                      AS severity,
  time::VARCHAR                 AS fired_at
FROM cloudtrail_events
WHERE class_uid     = 'okta'
  AND api_operation = 'user.session.start'
  AND auth_status   = 'failure';

CREATE MATERIALIZED VIEW IF NOT EXISTS det_m365_mass_external_sharing AS
SELECT
  CONCAT('det_m365_mass_external_sharing:', event_id) AS detection_id,
  event_id,
  COALESCE(actor_user_name, '') AS actor_user_name,
  COALESCE(cloud_region, '')    AS cloud_region,
  'medium'                      AS severity,
  time::VARCHAR                 AS fired_at
FROM cloudtrail_events
WHERE class_uid     = 'm365'
  AND api_operation IN ('SharingInvitationCreated', 'AddedToGroup',
                        'AnonymousLinkCreated', 'SharingSet');

CREATE MATERIALIZED VIEW IF NOT EXISTS det_m365_inbox_rule_auto_forward_external AS
SELECT
  CONCAT('det_m365_inbox_rule_auto_forward_external:', event_id) AS detection_id,
  event_id,
  COALESCE(actor_user_name, '') AS actor_user_name,
  COALESCE(cloud_region, '')    AS cloud_region,
  'high'                        AS severity,
  time::VARCHAR                 AS fired_at
FROM cloudtrail_events
WHERE class_uid     = 'm365'
  AND api_operation IN ('New-InboxRule', 'Set-InboxRule')
  AND (raw->>'parameters')::TEXT ILIKE '%ForwardTo%';
