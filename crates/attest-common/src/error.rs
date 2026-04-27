use thiserror::Error;

#[derive(Debug, Error)]
pub enum AttestError {
    #[error("normalization failed: {0}")]
    Normalization(String),

    #[error("serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("kafka error: {0}")]
    Kafka(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("not found: {0}")]
    NotFound(String),
}
