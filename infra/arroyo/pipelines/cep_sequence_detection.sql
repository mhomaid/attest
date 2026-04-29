-- cep_sequence_detection.sql
-- Arroyo CEP pipeline: detect multi-step attack sequences requiring
-- stateful event correlation that RisingWave cannot express cleanly.
--
-- Pattern: "console login THEN S3 access within 5 minutes by the same user"
-- Output:  alerts Redpanda topic (same schema as HELIQL detections)

-- Source: flat OCSF events from the cloudtrail Redpanda topic.
-- Column names match FlatEvent (attest-collector/src/producer.rs).
CREATE TABLE ocsf_events (
    event_id          TEXT,
    class_uid         TEXT,
    time              TIMESTAMP,
    tenant_id         TEXT,
    actor_user_name   TEXT,
    actor_user_uid    TEXT,
    cloud_region      TEXT,
    cloud_account_uid TEXT,
    severity          TEXT,
    auth_status       TEXT,
    api_operation     TEXT,
    api_service       TEXT
) WITH (
    connector = 'kafka',
    format = 'json',
    bootstrap_servers = 'redpanda:9092',
    topic = 'cloudtrail',
    type = 'source'
);

-- Sink: alerts topic consumed by orchestrator and workbench
CREATE TABLE alerts_sink (
    detection_id    TEXT,
    actor_user_name TEXT,
    cloud_region    TEXT,
    source_ip       TEXT,
    tenant_id       TEXT,
    severity        TEXT,
    mitre_id        TEXT,
    detail          TEXT,
    fired_at        TIMESTAMP
) WITH (
    connector = 'kafka',
    format = 'json',
    bootstrap_servers = 'redpanda:9092',
    topic = 'alerts',
    type = 'sink'
);

-- Anchor: successful console logins
-- auth_status = 'Success' is the casing produced by FlatEvent::from_ocsf
CREATE VIEW console_logins AS
SELECT
    event_id,
    actor_user_name,
    cloud_region,
    actor_user_uid AS source_ip,
    tenant_id,
    time AS login_time
FROM ocsf_events
WHERE api_operation = 'ConsoleLogin'
  AND auth_status = 'Success';

-- S3 data access events (any S3 read-like operation)
CREATE VIEW s3_access AS
SELECT
    event_id,
    actor_user_name,
    cloud_region,
    tenant_id,
    time AS access_time
FROM ocsf_events
WHERE api_operation IN ('GetObject', 'ListBuckets', 'GetBucketObject', 'GetObject_v2');

-- Detection: login THEN S3 access by the same user within 5 minutes
INSERT INTO alerts_sink
SELECT
    'aws_login_then_s3_access_sequence'             AS detection_id,
    l.actor_user_name,
    l.cloud_region,
    l.source_ip,
    l.tenant_id,
    'high'                                          AS severity,
    'T1078.004'                                     AS mitre_id,
    CONCAT(
        'User ', l.actor_user_name,
        ' logged in from ', l.cloud_region,
        ' at ', CAST(l.login_time AS TEXT),
        ' then accessed S3 at ', CAST(s.access_time AS TEXT)
    )                                               AS detail,
    s.access_time                                   AS fired_at
FROM console_logins l
JOIN s3_access s
  ON  l.actor_user_name = s.actor_user_name
  AND l.tenant_id       = s.tenant_id
  AND s.access_time > l.login_time
  AND s.access_time <= l.login_time + INTERVAL '5 minutes';
