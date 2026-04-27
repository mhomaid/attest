mod consumer;
mod error;
mod schema;
mod writer;

use anyhow::Result;
use clap::Parser;
use consumer::IcebergConsumer;
use object_store::aws::AmazonS3Builder;
use std::sync::Arc;
use tracing::info;
use writer::ParquetBatchWriter;

#[derive(Parser, Debug)]
#[command(name = "attest-storage-iceberg", about = "Redpanda → Parquet/Iceberg writer")]
struct Cli {
    #[arg(long, env = "KAFKA_BROKERS", default_value = "localhost:9092")]
    kafka_brokers: String,

    #[arg(long, env = "KAFKA_TOPIC", default_value = "cloudtrail")]
    kafka_topic: String,

    #[arg(long, env = "KAFKA_GROUP_ID", default_value = "attest-iceberg-writer")]
    kafka_group_id: String,

    /// S3 / MinIO endpoint URL
    #[arg(long, env = "S3_ENDPOINT", default_value = "http://minio:9000")]
    s3_endpoint: String,

    #[arg(long, env = "S3_BUCKET", default_value = "attest-warm")]
    s3_bucket: String,

    #[arg(long, env = "S3_ACCESS_KEY", default_value = "minioadmin")]
    s3_access_key: String,

    #[arg(long, env = "S3_SECRET_KEY", default_value = "minioadmin")]
    s3_secret_key: String,

    #[arg(long, env = "S3_REGION", default_value = "us-east-1")]
    s3_region: String,

    /// Prefix under the bucket for the Parquet files
    #[arg(long, env = "TABLE_PREFIX", default_value = "cloudtrail")]
    table_prefix: String,

    /// Number of events per Parquet batch
    #[arg(long, env = "BATCH_SIZE", default_value_t = 1000)]
    batch_size: usize,

    /// Maximum seconds between flushes regardless of batch size
    #[arg(long, env = "FLUSH_INTERVAL_SECS", default_value_t = 30)]
    flush_interval_secs: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "attest_storage_iceberg=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    info!(
        brokers = %cli.kafka_brokers,
        topic = %cli.kafka_topic,
        bucket = %cli.s3_bucket,
        endpoint = %cli.s3_endpoint,
        "attest-storage-iceberg starting"
    );

    let store = Arc::new(
        AmazonS3Builder::new()
            .with_endpoint(&cli.s3_endpoint)
            .with_bucket_name(&cli.s3_bucket)
            .with_access_key_id(&cli.s3_access_key)
            .with_secret_access_key(&cli.s3_secret_key)
            .with_region(&cli.s3_region)
            .with_allow_http(true)
            .build()?,
    );

    let writer = Arc::new(ParquetBatchWriter::new(store, &cli.table_prefix));

    let consumer = IcebergConsumer::new(
        &cli.kafka_brokers,
        &cli.kafka_group_id,
        &cli.kafka_topic,
        writer,
        cli.batch_size,
        cli.flush_interval_secs,
    )?;

    consumer.run().await
}
