//! Append-only, hash-chained attestation log — newline-delimited JSON.
//!
//! Each signed envelope stores `prev_hash` of the previous row. `attest verify`
//! walks the chain so a deleted or reordered line fails. External Merkle
//! anchoring is still planned.

use crate::envelope::{AttestationEnvelope, GENESIS_HASH};
use crate::signer::Signer;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

/// Thread-safe append-only writer. The mutex serializes `sign_and_append`
/// so two writers cannot claim the same predecessor.
#[derive(Clone)]
pub struct AttestationLog {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl AttestationLog {
    /// Create (or open) the log file at the given path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    /// Open from `ATTEST_LOG_PATH` env var, falling back to `./attestations.ndjson`.
    pub fn from_env() -> Self {
        let path =
            std::env::var("ATTEST_LOG_PATH").unwrap_or_else(|_| "./attestations.ndjson".into());
        Self::new(path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Hash the next envelope must point at (`GENESIS_HASH` when the log is empty).
    pub async fn last_chain_hash(&self) -> Result<String> {
        let _g = self.lock.lock().await;
        self.tip_unlocked().await
    }

    /// Set `prev_hash`, sign, and append under one lock.
    pub async fn sign_and_append(
        &self,
        signer: &Signer,
        envelope: &mut AttestationEnvelope,
    ) -> Result<()> {
        let _g = self.lock.lock().await;
        envelope.prev_hash = self.tip_unlocked().await?;
        signer.sign(envelope);
        self.write_line(envelope).await
    }

    /// Append an already-signed envelope as a single JSON line.
    pub async fn append(&self, envelope: &AttestationEnvelope) -> Result<()> {
        let _g = self.lock.lock().await;
        self.write_line(envelope).await
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

    async fn tip_unlocked(&self) -> Result<String> {
        let envs = self.read_all().await?;
        Ok(envs
            .last()
            .map(AttestationEnvelope::chain_hash)
            .unwrap_or_else(|| GENESIS_HASH.to_string()))
    }

    async fn write_line(&self, envelope: &AttestationEnvelope) -> Result<()> {
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
}
