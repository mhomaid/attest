use thiserror::Error;

#[derive(Debug, Error)]
pub enum IcebergWriterError {
    #[error("Kafka error: {0}")]
    Kafka(#[from] rdkafka::error::KafkaError),

    #[error("JSON deserialize error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    #[error("Object store error: {0}")]
    ObjectStore(#[from] object_store::Error),

    #[error("Iceberg error: {0}")]
    Iceberg(#[from] iceberg::Error),

    #[error("Anyhow: {0}")]
    Anyhow(#[from] anyhow::Error),
}
