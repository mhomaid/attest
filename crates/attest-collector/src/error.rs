use thiserror::Error;

#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("normalization failed: {0}")]
    Normalization(String),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("kafka error: {0}")]
    Kafka(String),

    #[error("s3 fetch failed: {0}")]
    S3(String),
}
