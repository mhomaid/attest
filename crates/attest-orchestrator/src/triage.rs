//! Core triage execution loop — Hybrid path (Classifier primary, LLM escalation).
//!
//! Phase 4b: EscalatedStub is replaced with a real `run_llm_loop` call.
//! EscalatedStub is kept only as a graceful fallback when the LLM call fails.
//!
//! Phase 5: After `run_llm_loop` returns a `TruePositive` verdict with a score
//! above `CROSS_REVIEW_SEVERITY_THRESHOLD`, a second "reviewer" LLM pass is
//! triggered. Disagreement downgrades the verdict to `NeedsInvestigation`.

use crate::agent::{AgentDefinition, ClassifierArtifact, ExecutionPath};
use crate::calibration::CalibrationClient;
use crate::guardrails::EnforcementMode;
use crate::llm_loop::{build_cross_review_block, run_llm_loop, run_review, LlmLoopError};
use crate::mcp_client::McpClient;
use anyhow::{Context, Result};
use attest_attestation::{
    AttestationEnvelope, AttestationLog, ClassifierEvidence, EscalationReason,
    EvidenceBlock, ExecutionPathKind, Signer, TimingBlock, Verdict,
};
use attest_feature_extractor::FeatureExtractor;
use attest_inference_router::ChatClient;
use attest_onnx_runtime::{NoveltyDetector, OnnxClassifier};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Severity threshold above which a `TruePositive` verdict triggers cross-review.
fn cross_review_threshold() -> f32 {
    std::env::var("CROSS_REVIEW_SEVERITY_THRESHOLD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.85)
}

/// Inbound triage request (JSON body for `POST /triage`).
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct TriageRequest {
    pub case_id: Option<Uuid>,
    pub tenant_id: Option<String>,
    /// Alert payload — either OCSF JSON or a flat feature map.
    pub alert: Value,
}

/// Verdict returned from the triage execution loop.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TriageVerdict {
    pub action_id: Uuid,
    pub case_id: Uuid,
    #[schema(value_type = String)]
    pub verdict: Verdict,
    #[schema(value_type = String)]
    pub execution_path: ExecutionPathKind,
    pub calibrated_confidence: f32,
    pub novelty_score: f32,
    /// True when the LLM escalation path was taken.
    pub escalated: bool,
    pub escalation_reason: Option<String>,
    pub latency_ms: u64,
    #[schema(value_type = Object)]
    pub classifier_evidence: Option<ClassifierEvidence>,
}

/// All runtime state needed by the triage loop — loaded once at orchestrator startup.
pub struct TriageEngine {
    pub agent: AgentDefinition,
    classifier: OnnxClassifier,
    novelty: NoveltyDetector,
    calibration: CalibrationClient,
    attestation_log: AttestationLog,
    signer: Arc<Signer>,
    /// Hex-encoded Ed25519 verifying key — published via `/agent` for offline verification.
    pub verifying_key: String,
    /// Pre-loaded system prompt text.
    pub system_prompt: String,
    /// SHA-256 hex of the system prompt (stored on `LlmEvidence`).
    pub system_prompt_hash: String,
    /// Pre-loaded reviewer system prompt text.
    pub reviewer_prompt: String,
    /// SHA-256 hex of the reviewer prompt.
    pub reviewer_prompt_hash: String,
    /// LLM chat client (Arc so it is shared across concurrent triage calls).
    llm_client: Option<Arc<Box<dyn ChatClient>>>,
    /// MCP gateway client.
    mcp_client: McpClient,
}

impl TriageEngine {
    /// Load all ONNX artifacts and build the engine.
    ///
    /// `system_prompt`, `system_prompt_hash`, `reviewer_prompt`, and
    /// `reviewer_prompt_hash` are passed in from `main` (hashes are computed
    /// at startup so they can be recorded on every envelope).
    pub fn load(
        agent: AgentDefinition,
        signer: Arc<Signer>,
        system_prompt: String,
        system_prompt_hash: String,
        reviewer_prompt: String,
        reviewer_prompt_hash: String,
        llm_client: Option<Box<dyn ChatClient>>,
    ) -> Result<Self> {
        let artifact = extract_classifier_artifact(&agent.execution)?;

        let classifier = OnnxClassifier::load(
            &artifact.model_path,
            artifact.shap_background_path.as_deref(),
        )
        .context("failed to load ONNX classifier")?;

        let novelty = NoveltyDetector::load(
            &artifact.novelty_mean_path,
            &artifact.novelty_inv_cov_path,
            &artifact.novelty_threshold_path,
        )
        .context("failed to load novelty detector")?;

        let verifying_key = signer.verifying_key_hex();

        Ok(Self {
            agent,
            classifier,
            novelty,
            calibration: CalibrationClient::from_env(),
            attestation_log: AttestationLog::from_env(),
            signer,
            verifying_key,
            system_prompt,
            system_prompt_hash,
            reviewer_prompt,
            reviewer_prompt_hash,
            llm_client: llm_client.map(|c| Arc::new(c)),
            mcp_client: McpClient::from_env(),
        })
    }

    /// Run the Hybrid triage loop and return a signed verdict.
    pub async fn run_triage(&self, req: TriageRequest) -> Result<TriageVerdict> {
        let t0 = Instant::now();
        let started_at = Utc::now();
        let action_id = Uuid::new_v4();
        let case_id = req.case_id.unwrap_or_else(Uuid::new_v4);
        let tenant_id = req.tenant_id.unwrap_or_else(|| "default".into());

        let artifact = extract_classifier_artifact(&self.agent.execution)?;

        // 1. Feature extraction
        let features = FeatureExtractor::extract_from_json(&req.alert);

        // 2. Classifier prediction + SHAP
        let (raw_score, shap_values) = self.classifier.predict(&features)
            .context("classifier inference failed")?;

        // 3. Calibration (classifier path)
        let case_class = infer_case_class(&req.alert);
        let calibrated = self.calibration.calibrate(&self.agent.id, &case_class, raw_score, "classifier").await;

        // 4. Novelty score
        let novelty_score = self.novelty.score(&features);

        // 5. Hybrid routing decision
        let should_escalate = calibrated < artifact.escalation_threshold
            || novelty_score > artifact.novelty_threshold;

        let (verdict, execution_path, evidence, escalation_reason) = if should_escalate {
            let reason = if novelty_score > artifact.novelty_threshold && calibrated < artifact.escalation_threshold {
                EscalationReason::BothLowConfidenceAndHighNovelty
            } else if novelty_score > artifact.novelty_threshold {
                EscalationReason::HighNoveltyScore {
                    score: novelty_score,
                    threshold: artifact.novelty_threshold,
                }
            } else {
                EscalationReason::LowCalibratedConfidence {
                    score: calibrated,
                    threshold: artifact.escalation_threshold,
                }
            };

            let classifier_draft = build_classifier_evidence(
                artifact, &features, &shap_values, raw_score, calibrated, novelty_score,
            );

            let reason_str = format!("{:?}", reason);

            tracing::info!(
                action_id = %action_id,
                case_id = %case_id,
                calibrated,
                novelty_score,
                reason = %reason_str,
                "escalating to LLM"
            );

            // Phase 4b: call the real LLM loop
            match self.try_llm_escalation(
                action_id,
                classifier_draft.clone(),
                reason.clone(),
                &req.alert,
            ).await {
                Ok((llm_verdict, hybrid_evidence)) => {
                    tracing::info!(
                        action_id = %action_id,
                        verdict = ?llm_verdict,
                        "LLM escalation produced Hybrid envelope"
                    );
                    (
                        llm_verdict,
                        ExecutionPathKind::Hybrid,
                        EvidenceBlock::Hybrid(hybrid_evidence),
                        Some(reason_str),
                    )
                }
                Err(e) => {
                    // Graceful fallback: keep the EscalatedStub so /triage never 500s
                    tracing::warn!(
                        action_id = %action_id,
                        error = %e,
                        error_debug = ?e,
                        "LLM escalation failed — falling back to EscalatedStub"
                    );
                    (
                        Verdict::EscalatedStub,
                        ExecutionPathKind::Hybrid,
                        EvidenceBlock::EscalatedStub {
                            classifier_draft,
                            escalation_reason: reason,
                        },
                        Some(reason_str),
                    )
                }
            }
        } else {
            // Confident classifier disposition
            let label = if calibrated >= 0.5 { Verdict::TruePositive } else { Verdict::Benign };
            let ev = build_classifier_evidence(
                artifact, &features, &shap_values, raw_score, calibrated, novelty_score,
            );
            (label, ExecutionPathKind::Classifier, EvidenceBlock::Classifier(ev), None)
        };

        let finished_at = Utc::now();
        let latency_ms = t0.elapsed().as_millis() as u64;

        // 6. Build and sign attestation envelope
        let mut envelope = AttestationEnvelope {
            envelope_version: "1.1".into(),
            agent_action_id: action_id,
            case_id,
            tenant_id,
            agent_id: self.agent.id.clone(),
            execution_path: execution_path.clone(),
            verdict: verdict.clone(),
            evidence,
            timing: TimingBlock { started_at, finished_at, total_ms: latency_ms },
            signature: String::new(),
            signed_at: finished_at,
        };
        self.signer.sign(&mut envelope);

        tracing::info!(
            action_id = %action_id,
            case_id = %case_id,
            verdict = ?verdict,
            path = ?execution_path,
            calibrated,
            novelty_score,
            latency_ms,
            "triage complete"
        );

        // 7. Append to log (non-blocking)
        let log = self.attestation_log.clone();
        let env_clone = envelope.clone();
        tokio::spawn(async move {
            if let Err(e) = log.append(&env_clone).await {
                tracing::error!(error = %e, "failed to write attestation log");
            }
        });

        Ok(TriageVerdict {
            action_id,
            case_id,
            verdict,
            execution_path,
            calibrated_confidence: calibrated,
            novelty_score,
            escalated: escalation_reason.is_some(),
            escalation_reason,
            latency_ms,
            classifier_evidence: match &envelope.evidence {
                EvidenceBlock::Classifier(ev) => Some(ev.clone()),
                EvidenceBlock::Hybrid(h) => Some(h.classifier_draft.clone()),
                EvidenceBlock::EscalatedStub { classifier_draft, .. } => Some(classifier_draft.clone()),
                _ => None,
            },
        })
    }

    /// Attempt LLM escalation; returns an error if no LLM client is configured or
    /// if the loop itself fails.
    async fn try_llm_escalation(
        &self,
        action_id: Uuid,
        classifier_draft: ClassifierEvidence,
        escalation_reason: EscalationReason,
        alert: &Value,
    ) -> Result<(Verdict, attest_attestation::HybridEvidence), LlmLoopError> {
        let client = self.llm_client.as_ref().ok_or_else(|| {
            LlmLoopError::Provider(anyhow::anyhow!(
                "no LLM client configured — set ATTEST_LLM_PROVIDER env var"
            ))
        })?;

        let max_iterations = self.llm_max_iterations();

        let mut result = run_llm_loop(
            client.as_ref().as_ref(),
            &self.mcp_client,
            &self.agent,
            &self.system_prompt,
            &self.system_prompt_hash,
            alert,
            classifier_draft,
            escalation_reason,
            max_iterations,
            action_id,
            None, // read enforcement mode from env
            None, // read max retries from env
        ).await?;

        // Phase 5: Cross-agent review for high-impact TruePositive verdicts
        if result.verdict == Verdict::TruePositive
            && EnforcementMode::from_env() == EnforcementMode::On
        {
            let calibrated = result.evidence.classifier_draft.calibrated_confidence;
            if calibrated >= cross_review_threshold() {
                tracing::info!(
                    action_id = %action_id,
                    calibrated,
                    "cross-review: triggering reviewer pass for high-impact TruePositive"
                );

                let last_reasoning = result.evidence.llm_final.intermediate_beliefs
                    .last()
                    .map(|b| b.content_summary.as_str())
                    .unwrap_or("");

                let outcome = run_review(
                    client.as_ref().as_ref(),
                    &self.reviewer_prompt,
                    &self.reviewer_prompt_hash,
                    "true_positive",
                    last_reasoning,
                    alert,
                ).await;

                let agrees = outcome.agrees;
                let cross_review_block = build_cross_review_block(
                    &self.reviewer_prompt_hash,
                    outcome,
                );

                result.evidence.llm_final.cross_review = Some(cross_review_block);

                if !agrees {
                    tracing::warn!(
                        action_id = %action_id,
                        "cross-review: reviewer disagreed — downgrading to NeedsInvestigation"
                    );
                    result.verdict = Verdict::NeedsInvestigation;
                }
            }
        }

        Ok((result.verdict, result.evidence))
    }

    fn llm_max_iterations(&self) -> u8 {
        match &self.agent.execution {
            ExecutionPath::Hybrid { escalation, .. } => match escalation.as_ref() {
                ExecutionPath::Llm { max_iterations, .. } => *max_iterations,
                _ => 8,
            },
            ExecutionPath::Llm { max_iterations, .. } => *max_iterations,
            _ => 8,
        }
    }
}

fn build_classifier_evidence(
    artifact: &ClassifierArtifact,
    features: &attest_feature_extractor::AlertFeatures,
    shap_values: &std::collections::HashMap<String, f64>,
    raw_prediction: f32,
    calibrated_confidence: f32,
    novelty_score: f32,
) -> ClassifierEvidence {
    ClassifierEvidence {
        model_artifact_hash: artifact.model_artifact_hash.clone(),
        feature_extractor_hash: artifact.feature_extractor_hash.clone(),
        input_features: features.to_named_map(),
        shap_values: shap_values.clone(),
        raw_prediction,
        calibrated_confidence,
        novelty_score,
    }
}

fn extract_classifier_artifact(execution: &ExecutionPath) -> Result<&ClassifierArtifact> {
    match execution {
        ExecutionPath::Classifier { artifact } => Ok(artifact),
        ExecutionPath::Hybrid { primary, .. } => match primary.as_ref() {
            ExecutionPath::Classifier { artifact } => Ok(artifact),
            _ => anyhow::bail!("Hybrid primary must be Classifier"),
        },
        _ => anyhow::bail!("expected Classifier or Hybrid execution path"),
    }
}

/// Infer a case class label from the alert JSON for calibration routing.
fn infer_case_class(alert: &Value) -> String {
    if let Some(s) = alert["case_class"].as_str() {
        return s.to_string();
    }
    let severity = alert["severity_score"].as_f64().unwrap_or(0.5);
    let rep = alert["entity_reputation_score"].as_f64().unwrap_or(0.0);
    let bd = alert["baseline_deviation"].as_f64().unwrap_or(0.0);
    if rep > 0.5 { return "brute_force".into(); }
    if bd > 3.0 { return "geo_anomaly".into(); }
    if severity >= 0.9 { return "exfiltration".into(); }
    "default".into()
}
