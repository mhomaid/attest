//! Append-only, hash-chained attestation log.
//!
//! Local default is newline-delimited JSON at `ATTEST_LOG_PATH`. When
//! `ATTEST_LOG_S3_BUCKET` is set, every append is also written as an object
//! (`{prefix}/{millis}_{action_id}.json`) plus a `tip.json` so a restarted
//! process can rebuild the chain from object storage.

use crate::envelope::{AttestationEnvelope, GENESIS_HASH};
use crate::signer::Signer;
use anyhow::{Context, Result};
use futures::StreamExt;
use object_store::aws::AmazonS3Builder;
use object_store::path::Path as ObjPath;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

#[derive(Clone)]
struct ObjectSink {
    store: Arc<dyn ObjectStore>,
    prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tip {
    hash: String,
    count: usize,
}

/// Thread-safe append-only writer. The mutex serializes `sign_and_append`
/// so two writers cannot claim the same predecessor.
#[derive(Clone)]
pub struct AttestationLog {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
    objects: Option<ObjectSink>,
}

impl AttestationLog {
    /// Create (or open) the log file at the given path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            lock: Arc::new(Mutex::new(())),
            objects: None,
        }
    }

    /// Dual-write to a local file and an `ObjectStore` (MinIO / S3 / in-memory).
    pub fn with_object_store(
        path: impl AsRef<Path>,
        store: Arc<dyn ObjectStore>,
        prefix: impl Into<String>,
    ) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            lock: Arc::new(Mutex::new(())),
            objects: Some(ObjectSink {
                store,
                prefix: prefix.into(),
            }),
        }
    }

    /// Open from `ATTEST_LOG_PATH` (default `./attestations.ndjson`).
    /// When `ATTEST_LOG_S3_BUCKET` is set, also dual-write to that bucket.
    pub fn from_env() -> Self {
        let path =
            std::env::var("ATTEST_LOG_PATH").unwrap_or_else(|_| "./attestations.ndjson".into());
        let log = Self::new(&path);
        match object_sink_from_env() {
            Ok(Some(sink)) => Self {
                objects: Some(sink),
                ..log
            },
            Ok(None) => log,
            Err(e) => {
                tracing::warn!(error = %e, "ATTEST_LOG_S3_BUCKET set but object store client failed");
                log
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn has_object_store(&self) -> bool {
        self.objects.is_some()
    }

    /// Hash the next envelope must point at (`GENESIS_HASH` when the log is empty).
    pub async fn last_chain_hash(&self) -> Result<String> {
        let _g = self.lock.lock().await;
        self.hydrate_unlocked().await?;
        Ok(self.tip_unlocked().await?.0)
    }

    /// Set `prev_hash`, sign, and append under one lock.
    pub async fn sign_and_append(
        &self,
        signer: &Signer,
        envelope: &mut AttestationEnvelope,
    ) -> Result<()> {
        let _g = self.lock.lock().await;
        self.hydrate_unlocked().await?;
        let (tip, seq) = self.tip_unlocked().await?;
        envelope.prev_hash = tip;
        signer.sign(envelope);
        self.write_line(envelope, seq).await
    }

    /// Append an already-signed envelope as a single JSON line.
    pub async fn append(&self, envelope: &AttestationEnvelope) -> Result<()> {
        let _g = self.lock.lock().await;
        self.hydrate_unlocked().await?;
        let (_, seq) = self.tip_unlocked().await?;
        self.write_line(envelope, seq).await
    }

    /// After a restart on a fresh disk, rebuild the local file from object storage
    /// so the next append chains onto the durable history instead of `GENESIS_HASH`.
    async fn hydrate_unlocked(&self) -> Result<()> {
        let Some(sink) = &self.objects else {
            return Ok(());
        };
        if self.path.exists() {
            return Ok(());
        }
        let envs = read_objects(sink).await?;
        if envs.is_empty() {
            return Ok(());
        }
        let mut body = String::new();
        for env in &envs {
            body.push_str(&serde_json::to_string(env)?);
            body.push('\n');
        }
        ensure_parent(&self.path).await;
        tokio::fs::write(&self.path, body)
            .await
            .context("rebuild local attestation log from object store")?;
        tracing::info!(
            count = envs.len(),
            "attestation log hydrated from object store"
        );
        Ok(())
    }

    /// Read all envelopes (for testing / replay). Returns in append order.
    ///
    /// When an object store is configured it is the source of truth — a local
    /// file can lag a successful durable write, or be missing after a restart.
    pub async fn read_all(&self) -> Result<Vec<AttestationEnvelope>> {
        if let Some(sink) = &self.objects {
            let from_obj = read_objects(sink).await?;
            if !from_obj.is_empty() {
                return Ok(from_obj);
            }
        }
        if self.path.exists() {
            return read_ndjson(&self.path).await;
        }
        Ok(vec![])
    }

    /// NDJSON bytes of the current log (file, or rebuilt from objects).
    pub async fn export_ndjson(&self) -> Result<String> {
        let envs = self.read_all().await?;
        let mut out = String::new();
        for env in envs {
            out.push_str(&serde_json::to_string(&env)?);
            out.push('\n');
        }
        Ok(out)
    }

    /// `(chain hash of the last row, number of rows)`.
    async fn tip_unlocked(&self) -> Result<(String, usize)> {
        let envs = self.read_all().await?;
        let hash = envs
            .last()
            .map(AttestationEnvelope::chain_hash)
            .unwrap_or_else(|| GENESIS_HASH.to_string());
        Ok((hash, envs.len()))
    }

    /// Object store first: if the durable write fails, nothing is appended locally
    /// and the chain tip does not move.
    async fn write_line(&self, envelope: &AttestationEnvelope, seq: usize) -> Result<()> {
        if let Some(sink) = &self.objects {
            put_envelope(sink, envelope, seq)
                .await
                .context("durable object-store write")?;
        }

        let mut line =
            serde_json::to_string(envelope).context("failed to serialise attestation envelope")?;
        line.push('\n');

        ensure_parent(&self.path).await;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .with_context(|| format!("failed to open attestation log at {:?}", self.path))?;

        file.write_all(line.as_bytes())
            .await
            .context("failed to write attestation envelope")?;
        file.sync_all()
            .await
            .context("failed to fsync attestation log")?;
        Ok(())
    }
}

async fn ensure_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
    }
}

fn object_sink_from_env() -> Result<Option<ObjectSink>> {
    let Ok(bucket) = std::env::var("ATTEST_LOG_S3_BUCKET") else {
        return Ok(None);
    };
    if bucket.trim().is_empty() {
        return Ok(None);
    }
    let prefix =
        std::env::var("ATTEST_LOG_S3_PREFIX").unwrap_or_else(|_| "attestations".to_string());
    let mut builder = AmazonS3Builder::from_env()
        .with_bucket_name(bucket)
        .with_region(std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".into()));
    if let Ok(endpoint) = std::env::var("AWS_ENDPOINT_URL") {
        builder = builder.with_endpoint(endpoint).with_allow_http(true);
    }
    let store = builder.build().context("attestation S3 client")?;
    Ok(Some(ObjectSink {
        store: Arc::new(store),
        prefix,
    }))
}

/// Keys are zero-padded sequence numbers so a lexical listing is append order,
/// even when two envelopes share a `signed_at` millisecond.
async fn put_envelope(sink: &ObjectSink, envelope: &AttestationEnvelope, seq: usize) -> Result<()> {
    let key = format!(
        "{}/{:020}_{}.json",
        sink.prefix.trim_end_matches('/'),
        seq,
        envelope.agent_action_id
    );
    let path = ObjPath::from(key);
    let bytes = serde_json::to_vec(envelope)?;
    sink.store
        .put(&path, PutPayload::from(bytes))
        .await
        .context("put envelope object")?;

    let tip = Tip {
        hash: envelope.chain_hash(),
        count: seq + 1,
    };
    let tip_path = ObjPath::from(format!("{}/tip.json", sink.prefix.trim_end_matches('/')));
    sink.store
        .put(&tip_path, PutPayload::from(serde_json::to_vec(&tip)?))
        .await
        .context("put tip.json")?;
    Ok(())
}

async fn read_objects(sink: &ObjectSink) -> Result<Vec<AttestationEnvelope>> {
    let prefix = ObjPath::from(sink.prefix.trim_end_matches('/').to_string());
    let mut stream = sink.store.list(Some(&prefix));
    let mut keys = Vec::new();
    while let Some(meta) = stream.next().await {
        let meta = meta.context("list attestation objects")?;
        let loc = meta.location.to_string();
        if loc.ends_with("tip.json") || !loc.ends_with(".json") {
            continue;
        }
        keys.push(meta.location);
    }
    keys.sort_by(|a, b| a.as_ref().cmp(b.as_ref()));

    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        let get = sink
            .store
            .get(&key)
            .await
            .with_context(|| format!("get {key}"))?;
        let bytes = get.bytes().await.context("read envelope object")?;
        let env: AttestationEnvelope =
            serde_json::from_slice(&bytes).context("envelope object JSON")?;
        out.push(env);
    }
    Ok(order_by_chain(out))
}

/// Walk `prev_hash` from genesis so a listing that is not lexical still
/// reconstructs append order (needed after a restart).
fn order_by_chain(envs: Vec<AttestationEnvelope>) -> Vec<AttestationEnvelope> {
    if envs.len() <= 1 {
        return envs;
    }
    use std::collections::HashMap;
    let mut by_prev: HashMap<String, AttestationEnvelope> = HashMap::new();
    let mut extra = Vec::new();
    for env in envs {
        if by_prev.insert(env.prev_hash.clone(), env.clone()).is_some() {
            extra.push(env);
        }
    }
    let mut out = Vec::with_capacity(by_prev.len());
    let mut cursor = GENESIS_HASH.to_string();
    while let Some(env) = by_prev.remove(&cursor) {
        cursor = env.chain_hash();
        out.push(env);
    }
    out.extend(by_prev.into_values());
    out.extend(extra);
    out
}

async fn read_ndjson(path: &Path) -> Result<Vec<AttestationEnvelope>> {
    let content = tokio::fs::read_to_string(path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::*;
    use chrono::Utc;
    use object_store::memory::InMemory;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn dummy(signer: &Signer, prev: &str) -> AttestationEnvelope {
        let now = Utc::now();
        let mut env = AttestationEnvelope {
            envelope_version: "1.1".into(),
            agent_action_id: Uuid::new_v4(),
            case_id: Uuid::new_v4(),
            tenant_id: "demo".into(),
            agent_id: "triager-hybrid-v1".into(),
            execution_path: ExecutionPathKind::Classifier,
            verdict: Verdict::Benign,
            evidence: EvidenceBlock::Classifier(ClassifierEvidence {
                model_artifact_hash: "aa".into(),
                feature_extractor_hash: "bb".into(),
                input_features: HashMap::new(),
                shap_values: HashMap::new(),
                raw_prediction: 0.1,
                calibrated_confidence: 0.1,
                novelty_score: 0.0,
            }),
            timing: TimingBlock {
                started_at: now,
                finished_at: now,
                total_ms: 1,
            },
            prev_hash: prev.to_string(),
            signature: String::new(),
            signed_at: now,
        };
        signer.sign(&mut env);
        env
    }

    static LOG_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[tokio::test]
    async fn object_store_survives_deleted_file() {
        let _guard = LOG_TEST_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("attest-log-{}", Uuid::new_v4()));
        let path = dir.join("attestations.ndjson");
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let prefix = format!("attestations-{}", Uuid::new_v4());
        let log = AttestationLog::with_object_store(&path, store.clone(), prefix);
        let signer = Signer::generate();

        let mut a = dummy(&signer, GENESIS_HASH);
        log.sign_and_append(&signer, &mut a).await.unwrap();
        let mut b = dummy(&signer, "");
        log.sign_and_append(&signer, &mut b).await.unwrap();

        tokio::fs::remove_file(&path).await.unwrap();
        let recovered = log.read_all().await.unwrap();
        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0].agent_action_id, a.agent_action_id);
        assert_eq!(recovered[1].prev_hash, a.chain_hash());
    }

    #[tokio::test]
    async fn append_after_restart_chains_onto_object_history() {
        let _guard = LOG_TEST_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("attest-log-{}", Uuid::new_v4()));
        let path = dir.join("attestations.ndjson");
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let signer = Signer::generate();

        let prefix = format!("attestations-{}", Uuid::new_v4());
        let before = AttestationLog::with_object_store(&path, store.clone(), &prefix);
        for _ in 0..3 {
            let mut env = dummy(&signer, "");
            before.sign_and_append(&signer, &mut env).await.unwrap();
        }

        tokio::fs::remove_file(&path).await.unwrap();
        let after = AttestationLog::with_object_store(&path, store, prefix);
        let mut next = dummy(&signer, "");
        after.sign_and_append(&signer, &mut next).await.unwrap();

        let all = after.read_all().await.unwrap();
        assert_eq!(all.len(), 4);
        let report = crate::verify::verify_log(&all, &signer.verifying_key_hex());
        assert!(report.all_ok(), "{:?}", report.results);
    }
}
