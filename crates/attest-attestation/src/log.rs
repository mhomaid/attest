//! Append-only attestation log — newline-delimited JSON to a local file.
//!
//! In production this would ship to S3/Blob storage with periodic Merkle-root
//! anchoring. For MVP we write to `ATTEST_LOG_PATH` (default `./attestations.ndjson`).

use crate::envelope::AttestationEnvelope;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

/// Thread-safe append-only writer.
#[derive(Clone)]
pub struct AttestationLog {
    path: PathBuf,
}

impl AttestationLog {
    /// Create (or open) the log file at the given path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Open from `ATTEST_LOG_PATH` env var, falling back to `./attestations.ndjson`.
    pub fn from_env() -> Self {
        let path =
            std::env::var("ATTEST_LOG_PATH").unwrap_or_else(|_| "./attestations.ndjson".into());
        Self::new(path)
    }

    /// Append an envelope as a single JSON line.
    pub async fn append(&self, envelope: &AttestationEnvelope) -> Result<()> {
        let mut line =
            serde_json::to_string(envelope).context("failed to serialise attestation envelope")?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .with_context(|| format!("failed to open attestation log at {:?}", self.path))?;

        file.write_all(line.as_bytes())
            .await
            .context("failed to write attestation envelope")?;
        Ok(())
    }

    /// Read all envelopes (for testing / replay). Returns in append order.
    pub async fn read_all(&self) -> Result<Vec<AttestationEnvelope>> {
        if !self.path.exists() {
            return Ok(vec![]);
        }
        let content = tokio::fs::read_to_string(&self.path)
            .await
            .context("failed to read attestation log")?;
        let mut out = Vec::new();
        for line in content.lines() {
            if line.is_empty() {
                continue;
            }
            let env: AttestationEnvelope =
                serde_json::from_str(line).context("failed to deserialise attestation envelope")?;
            out.push(env);
        }
        Ok(out)
    }
}
