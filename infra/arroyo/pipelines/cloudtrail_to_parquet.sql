-- cloudtrail_to_parquet.sql
-- Arroyo pipeline: Redpanda cloudtrail topic → Parquet files on MinIO/S3.
--
-- Parallel ETL path alongside attest-storage-iceberg (runs during migration).
-- After validation of identical output, attest-storage-iceberg will be deprecated.
--
-- Output:  s3://attest-warm/arroyo/cloudtrail/
-- Flush:   30-second inactivity interval OR 30-second rolling interval
-- Creds:   read from container env vars (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
-- MinIO:   path-style access via s3::http://minio:9000/ URL format

CREATE TABLE cloudtrail_source (
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

CREATE TABLE cloudtrail_parquet_sink (
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
    connector = 'filesystem',
    format = 'parquet',
    type = 'sink',
    path = 's3::http://minio:9000/attest-warm/arroyo/cloudtrail',
    'rolling_policy.interval' = interval '30 seconds',
    'rolling_policy.inactivity_interval' = interval '30 seconds'
);

INSERT INTO cloudtrail_parquet_sink
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
    api_service
FROM cloudtrail_source;
