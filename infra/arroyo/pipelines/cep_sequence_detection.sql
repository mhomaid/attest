-- cep_sequence_detection.sql
-- Arroyo CEP pipeline: detect multi-step attack sequences that require
-- stateful event correlation across time windows.
--
-- Pattern implemented here (template — extend for production):
--   "AWS console login from new region THEN S3 GetObject within 5 minutes"
--
-- Why Arroyo and not RisingWave for this:
--   RisingWave handles threshold and windowed aggregations well but does not
--   support ordered event-sequence matching (A then B then C with time
--   constraints and per-entity state). Arroyo's temporal join + windowed
--   aggregation covers this pattern declaratively.
--
-- Output: alerts topic on Redpanda, same schema as detections fired by
--         attest-detection-runtime. Downstream consumers (orchestrator,
--         workbench) are unaffected.

-- Source: all normalised OCSF events from the cloudtrail topic
CREATE TABLE ocsf_events (
    event_id          TEXT,
    actor_user_name   TEXT,
    cloud_region      TEXT,
    event_name        TEXT,   -- e.g. "ConsoleLogin", "GetObject", "PutObject"
    event_type        TEXT,   -- e.g. "Authentication", "CloudActivity"
    auth_status       TEXT,   -- "Success" | "Failure"
    source_ip         TEXT,
    tenant_id         TEXT,
    event_time        TIMESTAMP
) WITH (
    connector = 'kafka',
    format = 'json',
    'bootstrap.servers' = 'redpanda:9092',
    topic = 'cloudtrail',
    type = 'source',
    'group.id' = 'arroyo-cep-detector'
);

-- Sink: alerts topic consumed by orchestrator + workbench
CREATE TABLE alerts_sink (
    detection_id   TEXT,
    actor_user_name TEXT,
    cloud_region   TEXT,
    source_ip      TEXT,
    tenant_id      TEXT,
    severity       TEXT,
    mitre_id       TEXT,
    detail         TEXT,
    fired_at       TIMESTAMP
) WITH (
    connector = 'kafka',
    format = 'json',
    'bootstrap.servers' = 'redpanda:9092',
    topic = 'alerts',
    type = 'sink'
);

-- Step 1: successful console logins (anchor event)
CREATE VIEW console_logins AS
SELECT
    event_id,
    actor_user_name,
    cloud_region,
    source_ip,
    tenant_id,
    event_time AS login_time
FROM ocsf_events
WHERE event_name = 'ConsoleLogin'
  AND auth_status = 'Success';

-- Step 2: S3 data access events
CREATE VIEW s3_access AS
SELECT
    event_id,
    actor_user_name,
    cloud_region,
    tenant_id,
    event_time AS access_time
FROM ocsf_events
WHERE event_name IN ('GetObject', 'ListBuckets', 'GetBucketObject');

-- Detection: login THEN S3 access by the same user within 5 minutes.
-- Fires when we haven't seen the user log in from this region in the
-- prior 90 days (the baseline check is intentionally simplified here —
-- the full baseline lives in RisingWave and is queried by the orchestrator
-- for LLM-path investigations).
INSERT INTO alerts_sink
SELECT
    'aws_login_then_s3_access_sequence'        AS detection_id,
    l.actor_user_name,
    l.cloud_region,
    l.source_ip,
    l.tenant_id,
    'high'                                     AS severity,
    'T1078.004'                                AS mitre_id,
    CONCAT(
        'User ', l.actor_user_name,
        ' logged in from ', l.cloud_region,
        ' at ', CAST(l.login_time AS TEXT),
        ' then accessed S3 at ', CAST(s.access_time AS TEXT)
    )                                          AS detail,
    s.access_time                              AS fired_at
FROM console_logins l
JOIN s3_access s
  ON  l.actor_user_name = s.actor_user_name
  AND l.tenant_id       = s.tenant_id
  -- S3 access must happen AFTER login and within 5 minutes
  AND s.access_time > l.login_time
  AND s.access_time <= l.login_time + INTERVAL '5 minutes';
