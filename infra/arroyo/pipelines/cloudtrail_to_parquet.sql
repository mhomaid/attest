-- cloudtrail_to_parquet.sql
-- Arroyo pipeline: Redpanda cloudtrail topic → Parquet files on MinIO/S3.
--
-- This is the declarative equivalent of attest-storage-iceberg (the custom
-- Rust binary). Both run in parallel during the migration period. Once this
-- pipeline is validated to produce identical output, attest-storage-iceberg
-- will be deprecated.
--
-- Output path:  s3://attest-warm/arroyo/cloudtrail/<date>/<uuid>.parquet
-- Flush policy: every 30 seconds OR 1 000 rows, whichever comes first.
-- Credentials:  injected via AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY /
--               AWS_ENDPOINT environment variables on the Arroyo container.

CREATE TABLE cloudtrail_source (
    event_id          TEXT,
    actor_user_name   TEXT,
    cloud_region      TEXT,
    severity_id       INT,
    source_ip_address TEXT,
    event_name        TEXT,
    event_type        TEXT,
    tenant_id         TEXT,
    event_time        TIMESTAMP
) WITH (
    connector = 'kafka',
    format = 'json',
    'bootstrap.servers' = 'redpanda:9092',
    topic = 'cloudtrail',
    type = 'source',
    'group.id' = 'arroyo-cloudtrail-iceberg'
);

CREATE TABLE cloudtrail_parquet_sink (
    event_id          TEXT,
    actor_user_name   TEXT,
    cloud_region      TEXT,
    severity_id       INT,
    source_ip_address TEXT,
    event_name        TEXT,
    event_type        TEXT,
    tenant_id         TEXT,
    event_time        TIMESTAMP
) WITH (
    connector = 'filesystem',
    format = 'parquet',
    type = 'sink',
    path = 's3://attest-warm/arroyo/cloudtrail',
    rollover_seconds = '30',
    max_parts = '1000',
    -- Partition by date so ClickHouse Iceberg queries can prune by time
    'partition_fields' = 'event_time::DATE'
);

INSERT INTO cloudtrail_parquet_sink
SELECT
    event_id,
    actor_user_name,
    cloud_region,
    severity_id,
    source_ip_address,
    event_name,
    event_type,
    tenant_id,
    event_time
FROM cloudtrail_source;
