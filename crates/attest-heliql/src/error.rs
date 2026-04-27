use thiserror::Error;

#[derive(Debug, Error)]
pub enum HeliqlError {
    #[error("parse error: {0}")]
    ParseError(String),

    #[error("compile error: {0}")]
    CompileError(String),

    #[error("sigma conversion error: {0}")]
    SigmaConversionError(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
