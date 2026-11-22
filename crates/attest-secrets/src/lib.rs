//! `SecretSource`: one interface for runtime secrets (tool credentials today).
//!
//! * [`EnvSource`]  — dev only; reads process environment variables.
//! * [`FileSource`] — production default; reads files from a mounted directory
//!   (a Kubernetes Secret volume). Every `get` re-reads the file, so rotation is
//!   picked up without a restart.
//! * `AwsSecretsManagerSource` — behind the `aws` cargo feature.
//!
//! Secret values are wrapped in [`SecretValue`], whose `Debug` is redacted, and error
//! messages carry the secret *name*, never its value.

mod cache;
mod env;
mod file;
mod tool;
mod value;

#[cfg(feature = "aws")]
mod aws;

use std::sync::Arc;

use async_trait::async_trait;

pub use cache::CachedSource;
pub use env::EnvSource;
pub use file::FileSource;
pub use tool::ToolCredentials;
pub use value::SecretValue;

#[cfg(feature = "aws")]
pub use aws::AwsSecretsManagerSource;

/// Default mount path used by the Helm chart.
pub const DEFAULT_SECRETS_DIR: &str = "/var/run/attest/secrets";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret `{0}` not found")]
    NotFound(String),
    #[error("invalid secret name `{0}`")]
    InvalidName(String),
    #[error("secret `{0}` is empty")]
    Empty(String),
    #[error("secret `{0}` is too large")]
    TooLarge(String),
    #[error("secret backend error for `{name}`: {message}")]
    Backend { name: String, message: String },
    #[error("secret configuration error: {0}")]
    Config(String),
}

/// A place secrets can be read from. Implementations must not cache indefinitely:
/// callers rely on `get` observing rotation.
#[async_trait]
pub trait SecretSource: Send + Sync {
    async fn get(&self, name: &str) -> Result<SecretValue, SecretError>;
    /// Short label for logs (`env`, `file`, `aws`). Never contains secret material.
    fn kind(&self) -> &'static str;
}

/// Validate a secret name: 1..=128 chars of `[A-Za-z0-9._-]`, not starting with `.`.
/// This is what keeps `FileSource` inside its directory.
pub(crate) fn validate_name(name: &str) -> Result<(), SecretError> {
    let ok = !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if ok {
        Ok(())
    } else {
        Err(SecretError::InvalidName(name.escape_debug().to_string()))
    }
}

/// Build a source from configuration values (typically env vars
/// `ATTEST_SECRET_SOURCE`, `ATTEST_SECRETS_DIR`, `ATTEST_SECRETS_AWS_PREFIX`).
/// `kind` defaults to `env`. Unknown kinds and a missing `file` dir are errors.
pub fn source_from_config(
    kind: Option<&str>,
    dir: Option<&str>,
    aws_prefix: Option<&str>,
) -> Result<Arc<dyn SecretSource>, SecretError> {
    match kind
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .unwrap_or("env")
    {
        "env" => Ok(Arc::new(EnvSource::new("ATTEST_SECRET_"))),
        "file" => {
            let dir = dir
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .ok_or_else(|| {
                    SecretError::Config("file source requires a secrets directory".into())
                })?;
            Ok(Arc::new(FileSource::new(dir)))
        }
        "aws" => aws_source(aws_prefix),
        other => Err(SecretError::Config(format!(
            "unknown secret source `{}` (expected env, file or aws)",
            other.escape_debug()
        ))),
    }
}

#[cfg(feature = "aws")]
fn aws_source(prefix: Option<&str>) -> Result<Arc<dyn SecretSource>, SecretError> {
    // Client construction is async; building it lazily keeps this function sync.
    Ok(Arc::new(AwsSecretsManagerSource::lazy(
        prefix.unwrap_or("attest/").to_string(),
    )))
}

#[cfg(not(feature = "aws"))]
fn aws_source(_prefix: Option<&str>) -> Result<Arc<dyn SecretSource>, SecretError> {
    Err(SecretError::Config(
        "aws secret source requires building with the `aws` feature".into(),
    ))
}
