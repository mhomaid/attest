use std::io::ErrorKind;
use std::path::PathBuf;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;

use crate::{validate_name, SecretError, SecretSource, SecretValue};

const MAX_SECRET_BYTES: u64 = 64 * 1024;

/// Reads `<dir>/<name>`: the layout of a mounted Kubernetes Secret volume. Each `get`
/// re-reads the file, so when the kubelet swaps in a rotated value the next call sees
/// it. Trailing newlines are trimmed.
pub struct FileSource {
    dir: PathBuf,
}

impl FileSource {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

#[async_trait]
impl SecretSource for FileSource {
    async fn get(&self, name: &str) -> Result<SecretValue, SecretError> {
        validate_name(name)?;
        let backend = |message: String| SecretError::Backend {
            name: name.to_string(),
            message,
        };
        let file = match tokio::fs::File::open(self.dir.join(name)).await {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Err(SecretError::NotFound(name.to_string()))
            }
            Err(e) => return Err(backend(e.kind().to_string())),
        };
        // Bounded read: never slurp an arbitrarily large file.
        let mut buf = Vec::new();
        file.take(MAX_SECRET_BYTES + 1)
            .read_to_end(&mut buf)
            .await
            .map_err(|e| backend(e.kind().to_string()))?;
        if buf.len() as u64 > MAX_SECRET_BYTES {
            return Err(SecretError::TooLarge(name.to_string()));
        }
        let text = String::from_utf8(buf).map_err(|_| backend("not valid UTF-8".into()))?;
        let trimmed = text.trim_end_matches(['\n', '\r']);
        if trimmed.is_empty() {
            return Err(SecretError::Empty(name.to_string()));
        }
        Ok(SecretValue::new(trimmed))
    }

    fn kind(&self) -> &'static str {
        "file"
    }
}
