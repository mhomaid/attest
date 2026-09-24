mod catalog;
mod consumer;
mod schema;
mod storage;
mod writer;

use anyhow::Result;
use catalog::{open_or_create_table, S3Settings, Warehouse};
use clap::Parser;
use consumer::IcebergConsumer;
use std::sync::Arc;
use tracing::info;
use writer::IcebergBatchWriter;

#[derive(Parser, Debug)]
#[command(
    name = "attest-storage-iceberg",
    about = "Redpanda → Parquet + Iceberg snapshot writer"
)]
struct Cli {
    #[arg(long, env = "KAFKA_BROKERS", default_value = "localhost:9092")]
    kafka_brokers: String,

    #[arg(long, env = "KAFKA_TOPIC", default_value = "cloudtrail")]
    kafka_topic: String,

    #[arg(long, env = "KAFKA_GROUP_ID", default_value = "attest-iceberg-writer")]
    kafka_group_id: String,

    /// S3-compatible endpoint. Empty = AWS default (IRSA / instance role).
    #[arg(long, env = "S3_ENDPOINT", default_value = "")]
    s3_endpoint: String,

    #[arg(long, env = "S3_BUCKET", default_value = "attest-warm")]
    s3_bucket: String,

    /// Static keys. Empty = default AWS credential chain (IRSA).
    #[arg(long, env = "S3_ACCESS_KEY", default_value = "")]
    s3_access_key: String,

    #[arg(long, env = "S3_SECRET_KEY", default_value = "")]
    s3_secret_key: String,

    #[arg(long, env = "S3_REGION", default_value = "us-east-1")]
    s3_region: String,

    /// Iceberg warehouse. `s3://bucket/prefix` (default) or a local directory.
    #[arg(long, env = "ICEBERG_WAREHOUSE", default_value = "s3://attest-warm")]
    iceberg_warehouse: String,

    /// Table name under the `attest` namespace.
    #[arg(long, env = "TABLE_PREFIX", default_value = "cloudtrail")]
    table_prefix: String,

    /// Number of events per Parquet / snapshot batch
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

    let warehouse = if cli.iceberg_warehouse.starts_with("s3://")
        || cli.iceberg_warehouse.starts_with("s3a://")
    {
        let prefix = cli
            .iceberg_warehouse
            .splitn(4, '/')
            .nth(3)
            .unwrap_or("")
            .to_string();
        Warehouse::s3(
            S3Settings {
                endpoint: cli.s3_endpoint.clone(),
                bucket: cli.s3_bucket.clone(),
                access_key: cli.s3_access_key.clone(),
                secret_key: cli.s3_secret_key.clone(),
                region: cli.s3_region.clone(),
            },
            &prefix,
        )
    } else {
        Warehouse::local_fs(&cli.iceberg_warehouse)?
    };

    info!(
        brokers = %cli.kafka_brokers,
        topic = %cli.kafka_topic,
        warehouse = %warehouse.location,
        table = %cli.table_prefix,
        "attest-storage-iceberg starting"
    );

    let open = open_or_create_table(warehouse, &cli.table_prefix).await?;
    let writer = Arc::new(IcebergBatchWriter::from_open(open));

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
