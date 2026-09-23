//! Offline verification and classifier replay for signed attestation envelopes.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use attest_attestation::{
    AttestationEnvelope, ClassifierEvidence, EvidenceBlock, ExecutionPathKind, LlmEvidence, Signer,
    Verdict,
};

pub use attest_attestation::{verify_log, LineResult, VerifyReport};
use attest_feature_extractor::AlertFeatures;
use attest_onnx_runtime::OnnxClassifier;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const PREDICT_TOLERANCE: f32 = 1e-6;

/// Read an NDJSON attestation log. Blank lines are skipped.
pub fn read_log(path: &Path) -> Result<Vec<AttestationEnvelope>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read attestation log {}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let env: AttestationEnvelope = serde_json::from_str(line)
            .with_context(|| format!("{}:{}: invalid envelope JSON", path.display(), i + 1))?;
        out.push(env);
    }
    Ok(out)
}

/// Published artifact hashes every envelope must match (`--pin-model`, `--pin-prompt`).
#[derive(Debug, Clone, Default)]
pub struct Pins {
    pub model_artifact_hash: Option<String>,
    pub system_prompt_hash: Option<String>,
}

impl Pins {
    pub fn is_empty(&self) -> bool {
        self.model_artifact_hash.is_none() && self.system_prompt_hash.is_none()
    }
}

/// Fail rows that verified but were produced by a model or prompt other than the pinned one.
pub fn apply_pins(report: &mut VerifyReport, log: &[AttestationEnvelope], pins: &Pins) {
    if pins.is_empty() {
        return;
    }
    for (row, env) in report.results.iter_mut().zip(log) {
        if !row.ok {
            continue;
        }
        let pin = attest_attestation::AgentPin {
            agent_id: env.agent_id.clone(),
            model_artifact_hash: pins.model_artifact_hash.clone(),
            system_prompt_hash: pins.system_prompt_hash.clone(),
        };
        if let Err(e) = attest_attestation::check_pin(env, &pin) {
            row.ok = false;
            row.detail = e.to_string();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayReport {
    pub action_id: Uuid,
    pub path: String,
    pub lines: Vec<String>,
    pub ok: bool,
}

/// Replay one envelope. Classifier / auto-close / hybrid-classifier drafts are
/// re-inferred against `--model`. LLM evidence is integrity-checked only.
pub fn replay(
    envelope: &AttestationEnvelope,
    verifying_key_hex: &str,
    model_path: Option<&Path>,
) -> Result<ReplayReport> {
    let mut lines = Vec::new();
    let mut ok = true;

    match Signer::verify(envelope, verifying_key_hex) {
        Ok(true) => lines.push("signature: pass".into()),
        Ok(false) => {
            lines.push("signature: FAIL".into());
            ok = false;
        }
        Err(e) => {
            lines.push(format!("signature: FAIL ({e})"));
            ok = false;
        }
    }

    if let Some(llm) = llm_evidence(&envelope.evidence) {
        let (llm_ok, llm_lines) = replay_llm(llm);
        lines.extend(llm_lines);
        ok &= llm_ok;
        if classifier_evidence(&envelope.evidence).is_none() {
            lines.push(
                "mode: integrity only — LLM outputs are not regenerated (not deterministic)".into(),
            );
        }
    }

    if let Some(ev) = classifier_evidence(&envelope.evidence) {
        let model = model_path.ok_or_else(|| {
            anyhow::anyhow!("--model is required to replay a classifier decision")
        })?;
        let (cls_ok, cls_lines) = replay_classifier(envelope, ev, model)?;
        lines.extend(cls_lines);
        ok &= cls_ok;
        lines.push("mode: deterministic classifier replay".into());
    }

    if classifier_evidence(&envelope.evidence).is_none()
        && llm_evidence(&envelope.evidence).is_none()
    {
        lines.push("mode: signature verification only (no classifier or LLM evidence)".into());
    }

    Ok(ReplayReport {
        action_id: envelope.agent_action_id,
        path: path_label(&envelope.execution_path),
        lines,
        ok,
    })
}

fn replay_classifier(
    envelope: &AttestationEnvelope,
    ev: &ClassifierEvidence,
    model_path: &Path,
) -> Result<(bool, Vec<String>)> {
    let mut lines = Vec::new();
    let mut ok = true;

    let bytes = fs::read(model_path)
        .with_context(|| format!("failed to read model {}", model_path.display()))?;
    let model_hash = hex::encode(Sha256::digest(&bytes));
    if model_hash == ev.model_artifact_hash {
        lines.push(format!("model_hash: pass ({model_hash})"));
    } else {
        lines.push(format!(
            "model_hash: FAIL recorded={} file={model_hash}",
            ev.model_artifact_hash
        ));
        ok = false;
    }

    let expected_feat = AlertFeatures::column_order_hash();
    if ev.feature_extractor_hash == expected_feat {
        lines.push(format!("feature_hash: pass ({expected_feat})"));
    } else {
        lines.push(format!(
            "feature_hash: FAIL recorded={} expected={expected_feat}",
            ev.feature_extractor_hash
        ));
        ok = false;
    }

    let features = AlertFeatures::from_named_map(&ev.input_features)
        .map_err(|e| anyhow::anyhow!("recorded features: {e}"))?;
    let classifier = OnnxClassifier::load(model_path, None)?;
    let (replayed, _) = classifier.predict(&features)?;
    let delta = (replayed - ev.raw_prediction).abs();
    if delta <= PREDICT_TOLERANCE {
        lines.push(format!(
            "raw_prediction: pass recorded={} replayed={replayed} delta={delta}",
            ev.raw_prediction
        ));
    } else {
        lines.push(format!(
            "raw_prediction: FAIL recorded={} replayed={replayed} delta={delta} (tolerance {PREDICT_TOLERANCE})",
            ev.raw_prediction
        ));
        ok = false;
    }

    if matches!(envelope.verdict, Verdict::TruePositive | Verdict::Benign) {
        let inferred = if replayed >= 0.5 {
            Verdict::TruePositive
        } else {
            Verdict::Benign
        };
        if inferred == envelope.verdict {
            lines.push(format!("verdict: pass ({:?})", envelope.verdict));
        } else {
            lines.push(format!(
                "verdict: FAIL recorded={:?} inferred_from_replay={inferred:?}",
                envelope.verdict
            ));
            ok = false;
        }
    } else {
        lines.push(format!(
            "verdict: recorded={:?} (not re-derived; path is not a confident classifier close)",
            envelope.verdict
        ));
    }

    Ok((ok, lines))
}

fn replay_llm(ev: &LlmEvidence) -> (bool, Vec<String>) {
    let mut lines = Vec::new();
    let mut ok = true;

    if ev.system_prompt_hash.is_empty() {
        lines.push("system_prompt_hash: FAIL (empty)".into());
        ok = false;
    } else {
        lines.push(format!(
            "system_prompt_hash: present ({})",
            ev.system_prompt_hash
        ));
    }

    let ordered = ev
        .tool_calls
        .windows(2)
        .all(|w| w[0].timestamp <= w[1].timestamp);
    if ordered {
        lines.push(format!(
            "tool_call_chain: pass ({} calls, chronological)",
            ev.tool_calls.len()
        ));
    } else {
        lines.push("tool_call_chain: FAIL (timestamps not in order)".into());
        ok = false;
    }

    (ok, lines)
}

pub fn classifier_evidence(block: &EvidenceBlock) -> Option<&ClassifierEvidence> {
    match block {
        EvidenceBlock::Classifier(ev) => Some(ev),
        EvidenceBlock::Hybrid(h) => Some(&h.classifier_draft),
        EvidenceBlock::EscalatedStub {
            classifier_draft, ..
        } => Some(classifier_draft),
        EvidenceBlock::AutoClose(ac) => Some(&ac.classifier_evidence),
        EvidenceBlock::Llm(_) | EvidenceBlock::Override(_) => None,
    }
}

pub fn llm_evidence(block: &EvidenceBlock) -> Option<&LlmEvidence> {
    match block {
        EvidenceBlock::Llm(ev) => Some(ev),
        EvidenceBlock::Hybrid(h) => Some(&h.llm_final),
        _ => None,
    }
}

pub fn find_envelope<'a>(
    log: &'a [AttestationEnvelope],
    action_id: &Uuid,
) -> Result<&'a AttestationEnvelope> {
    log.iter()
        .find(|e| e.agent_action_id == *action_id)
        .ok_or_else(|| anyhow::anyhow!("no envelope with agent_action_id {action_id}"))
}

fn path_label(path: &ExecutionPathKind) -> String {
    match path {
        ExecutionPathKind::Classifier => "classifier".into(),
        ExecutionPathKind::Llm => "llm".into(),
        ExecutionPathKind::Hybrid => "hybrid".into(),
        ExecutionPathKind::HumanOverride => "human_override".into(),
    }
}

/// SHA-256 hex of a file — same algorithm as `ml/triager/train.py`.
pub fn file_sha256_hex(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

pub fn write_report_verify(report: &VerifyReport) {
    for r in &report.results {
        let tag = if r.ok { "PASS" } else { "FAIL" };
        println!("{tag}  {}  {}", r.action_id, r.detail);
    }
    println!("verified {}/{}", report.passed(), report.results.len());
}

pub fn write_report_replay(report: &ReplayReport) {
    println!("action_id: {}", report.action_id);
    println!("path: {}", report.path);
    for line in &report.lines {
        println!("{line}");
    }
    println!("RESULT: {}", if report.ok { "PASS" } else { "FAIL" });
}

/// Exit 1 when any envelope failed. Used by both subcommands.
pub fn exit_code(ok: bool) -> i32 {
    if ok {
        0
    } else {
        1
    }
}

pub fn parse_action_id(s: &str) -> Result<Uuid> {
    s.parse().with_context(|| format!("`{s}` is not a UUID"))
}

/// Resolve the verifying key from `--key`, `--key-file`, or `ATTEST_VERIFYING_KEY`.
/// Prefer `--key-file` so the hex never lands in the process list or shell history.
pub fn resolve_verifying_key(key: Option<String>, key_file: Option<&Path>) -> Result<String> {
    if let Some(path) = key_file {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read key file {}", path.display()))?;
        let hex = raw.trim();
        if hex.is_empty() {
            anyhow::bail!("{} is empty", path.display());
        }
        return Ok(hex.to_string());
    }
    let hex = key
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("pass --key-file PATH, --key HEX, or set ATTEST_VERIFYING_KEY")
        })?;
    Ok(hex)
}

#[cfg(test)]
mod resolve_key_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn key_file_wins_and_trims() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("key.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "  abcdef  ").unwrap();
        assert_eq!(
            resolve_verifying_key(Some("ignored".into()), Some(&path)).unwrap(),
            "abcdef"
        );
    }

    #[test]
    fn rejects_empty_key() {
        assert!(resolve_verifying_key(None, None).is_err());
        assert!(resolve_verifying_key(Some("   ".into()), None).is_err());
    }
}
