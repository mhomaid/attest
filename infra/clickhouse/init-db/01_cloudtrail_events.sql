-- Auto-created on ClickHouse startup.
-- Points at the Parquet files written by attest-storage-iceberg into Garage (attest-warm bucket).
-- The key pair is the local dev default from docker-compose.yml.
CREATE TABLE IF NOT EXISTS cloudtrail_events
(
    event_id          String,
    class_uid         Nullable(String),
    time              Nullable(DateTime64(6, 'UTC')),
    tenant_id         Nullable(String),
    actor_user_name   Nullable(String),
    actor_user_uid    Nullable(String),
    cloud_region      Nullable(String),
    cloud_account_uid Nullable(String),
    severity          Nullable(String),
    auth_status       Nullable(String),
    api_operation     Nullable(String),
    api_service       Nullable(String),
    raw               Nullable(String)
)
ENGINE = S3('http://garage:3900/attest-warm/attest/cloudtrail/data/*.parquet', 'GK0123456789abcdef0123456789abcdef', '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef', 'Parquet');
