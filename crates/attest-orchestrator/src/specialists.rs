//! Hunter and Responder specialists.
//!
//! Hunter is an LLM path (warm-tier hunt + optional detection PR). Responder is
//! a constrained LLM path whose actions are always gated by the shadow check
//! and the policy engine. Both have a deterministic fallback when no LLM is
//! configured so local/CI can still produce a signed envelope.

use crate::llm_loop::{run_specialist_llm_loop, LlmLoopError, SpecialistSpec};
use crate::mcp_client::McpClient;
use crate::shadow_check::ShadowChecker;
use anyhow::Result;
use attest_attestation::{
    AttestationEnvelope, AttestationLog, EvidenceBlock, ExecutionPathKind, IntermediateBelief,
    LlmEvidence, ShadowCheckDecision, Signer, TimingBlock, Verdict,
};
use attest_inference_router::{ChatClient, ToolDef};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct HuntRequest {
    pub case_id: Option<Uuid>,
    pub tenant_id: Option<String>,
    pub hypothesis: String,
    #[serde(default)]
    pub alert: Value,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct HuntResult {
    pub action_id: Uuid,
    pub case_id: Uuid,
    #[schema(value_type = String)]
    pub verdict: Verdict,
    pub queried_warm_tier: bool,
    pub latency_ms: u64,
    pub fallback: bool,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RespondRequest {
    pub case_id: Option<Uuid>,
    pub tenant_id: Option<String>,
    /// One of `idp_revoke_session`, `edr_isolate_host`, `firewall_block_ioc`.
    pub action: String,
    pub target: String,
    #[serde(default = "default_confidence")]
    pub calibrated_confidence: f32,
    #[serde(default = "default_blast")]
    pub blast_radius: u32,
    #[serde(default)]
    pub alert: Value,
}

fn default_confidence() -> f32 {
    0.90
}
fn default_blast() -> u32 {
    1
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RespondResult {
    pub action_id: Uuid,
    pub case_id: Uuid,
    pub action: String,
    pub target: String,
    pub executed: bool,
    #[schema(value_type = Object)]
    pub shadow_check: ShadowCheckDecision,
    pub latency_ms: u64,
    pub fallback: bool,
}

pub fn hunter_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "query_warm_tier".into(),
            description: "Query historical events from the Iceberg warm tier.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "sql": { "type": "string" },
                    "limit": { "type": "integer", "default": 500 }
                },
                "required": ["sql"]
            }),
        },
        ToolDef {
            name: "lookup_threat_intel".into(),
            description: "Look up an indicator.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "indicator": { "type": "string" },
                    "indicator_type": { "type": "string" }
                },
                "required": ["indicator", "indicator_type"]
            }),
        },
        ToolDef {
            name: "propose_detection_pr".into(),
            description: "Propose a HELIQL detection as a draft PR (does not merge).".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "heliql": { "type": "string" },
                    "rationale": { "type": "string" }
                },
                "required": ["title", "heliql"]
            }),
        },
        ToolDef {
            name: "request_human_review".into(),
            description: "Escalate a hunt finding to a human analyst.".into(),
            parameters: json!({
                "type": "object",
                "properties": { "reason": { "type": "string" } },
                "required": ["reason"]
            }),
        },
    ]
}

pub fn responder_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "idp_revoke_session".into(),
            description: "Revoke an IdP session. Policy-gated; never runs unaudited.".into(),
            parameters: json!({
                "type": "object",
                "properties": { "principal": { "type": "string" } },
                "required": ["principal"]
            }),
        },
        ToolDef {
            name: "edr_isolate_host".into(),
            description: "Isolate a host via EDR. Policy-gated.".into(),
            parameters: json!({
                "type": "object",
                "properties": { "host": { "type": "string" } },
                "required": ["host"]
            }),
        },
        ToolDef {
            name: "firewall_block_ioc".into(),
            description: "Block an IOC at the firewall.".into(),
            parameters: json!({
                "type": "object",
                "properties": { "indicator": { "type": "string" } },
                "required": ["indicator"]
            }),
        },
    ]
}

#[allow(clippy::too_many_arguments)]
pub async fn run_hunt(
    req: HuntRequest,
    llm: Option<&dyn ChatClient>,
    mcp: &McpClient,
    signer: &Signer,
    log: &AttestationLog,
    system_prompt: &str,
    system_prompt_hash: &str,
    agent_id: &str,
) -> Result<HuntResult> {
    let t0 = Instant::now();
    let started_at = Utc::now();
    let action_id = Uuid::new_v4();
    let case_id = req.case_id.unwrap_or_else(Uuid::new_v4);
    let tenant_id = req.tenant_id.unwrap_or_else(|| "default".into());

    let (evidence, verdict, fallback) = if let Some(client) = llm {
        let user = format!(
            "Hunt hypothesis:\n{}\n\nAlert JSON:\n{}\n\n\
             Call query_warm_tier (and other tools) before any verdict. \
             Final JSON: {{\"verdict\":\"...\",\"confidence\":0.0,\"reasoning\":\"...\",\"evidence_citations\":[]}}",
            req.hypothesis,
            serde_json::to_string_pretty(&req.alert).unwrap_or_else(|_| req.alert.to_string())
        );
        match run_specialist_llm_loop(
            client,
            mcp,
            SpecialistSpec {
                role: "hunter",
                agent_id: agent_id.to_string(),
                tools: hunter_tools(),
                require_retrieval: true,
            },
            system_prompt,
            system_prompt_hash,
            &user,
            0.5,
            8,
            action_id,
        )
        .await
        {
            Ok(out) => (EvidenceBlock::Llm(out.evidence), out.verdict, false),
            Err(LlmLoopError::Provider(e)) => {
                tracing::warn!(error = %e, "hunter LLM failed — deterministic fallback");
                deterministic_hunt_evidence(system_prompt_hash, &req.hypothesis)
            }
            Err(e) => return Err(e.into()),
        }
    } else {
        deterministic_hunt_evidence(system_prompt_hash, &req.hypothesis)
    };

    let queried_warm = match &evidence {
        EvidenceBlock::Llm(ev) => ev.tool_calls.iter().any(|t| t.tool_id == "query_warm_tier"),
        _ => false,
    };
    let latency_ms = t0.elapsed().as_millis() as u64;
    let finished_at = Utc::now();
    let mut envelope = AttestationEnvelope {
        envelope_version: "1.1".into(),
        agent_action_id: action_id,
        case_id,
        tenant_id,
        agent_id: agent_id.to_string(),
        execution_path: ExecutionPathKind::Llm,
        verdict: verdict.clone(),
        evidence,
        timing: TimingBlock {
            started_at,
            finished_at,
            total_ms: latency_ms,
        },
        prev_hash: String::new(),
        signature: String::new(),
        signed_at: finished_at,
    };
    log.sign_and_append(signer, &mut envelope).await?;

    Ok(HuntResult {
        action_id,
        case_id,
        verdict,
        queried_warm_tier: queried_warm,
        latency_ms,
        fallback,
    })
}

fn deterministic_hunt_evidence(
    prompt_hash: &str,
    hypothesis: &str,
) -> (EvidenceBlock, Verdict, bool) {
    let evidence = LlmEvidence {
        model_provider: "deterministic".into(),
        model_id: "hunter-fallback".into(),
        model_version_hash: hex::encode(Sha256::digest(b"hunter-fallback")),
        system_prompt_hash: prompt_hash.to_string(),
        tool_calls: vec![],
        intermediate_beliefs: vec![IntermediateBelief {
            iteration: 0,
            content_summary: format!("fallback hunt plan for: {hypothesis}"),
            self_reported_confidence: Some(0.0),
            timestamp: Utc::now(),
        }],
        evidence_citations: vec![],
        total_iterations: 0,
        validation_retries: 0,
        cross_review: None,
    };
    (
        EvidenceBlock::Llm(evidence),
        Verdict::NeedsInvestigation,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
pub async fn run_respond(
    req: RespondRequest,
    llm: Option<&dyn ChatClient>,
    mcp: &McpClient,
    signer: &Signer,
    log: &AttestationLog,
    shadow: &ShadowChecker,
    system_prompt: &str,
    system_prompt_hash: &str,
    agent_id: &str,
) -> Result<RespondResult> {
    let t0 = Instant::now();
    let started_at = Utc::now();
    let action_id = Uuid::new_v4();
    let case_id = req.case_id.unwrap_or_else(Uuid::new_v4);
    let tenant_id = req.tenant_id.unwrap_or_else(|| "default".into());

    let shadow_check = shadow.check_responder(
        &req.action,
        &req.target,
        req.calibrated_confidence,
        req.blast_radius,
    );

    let mut executed = false;
    let mut fallback = true;
    let mut tool_records = vec![];

    if shadow_check.allowed {
        if let Some(client) = llm {
            let user = format!(
                "Propose the containment action `{}` against `{}`.\n\
                 Shadow check already allowed this action. Call the matching tool, then emit JSON:\n\
                 {{\"verdict\":\"true_positive\",\"confidence\":{},\"reasoning\":\"...\",\"evidence_citations\":[]}}\n\
                 Alert:\n{}",
                req.action,
                req.target,
                req.calibrated_confidence,
                serde_json::to_string_pretty(&req.alert).unwrap_or_else(|_| req.alert.to_string())
            );
            match run_specialist_llm_loop(
                client,
                mcp,
                SpecialistSpec {
                    role: "responder",
                    agent_id: agent_id.to_string(),
                    tools: responder_tools(),
                    require_retrieval: false,
                },
                system_prompt,
                system_prompt_hash,
                &user,
                req.calibrated_confidence,
                6,
                action_id,
            )
            .await
            {
                Ok(out) => {
                    fallback = false;
                    executed = out
                        .evidence
                        .tool_calls
                        .iter()
                        .any(|t| t.tool_id == req.action && t.policy_decision == "allow");
                    tool_records = out.evidence.tool_calls;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "responder LLM failed — recording planned action only");
                }
            }
        }
        if fallback {
            // Record a planned (not live) action so the envelope still exists.
            executed = false;
        }
    }

    let latency_ms = t0.elapsed().as_millis() as u64;
    let finished_at = Utc::now();
    let evidence = LlmEvidence {
        model_provider: if fallback {
            "deterministic".into()
        } else {
            "llm".into()
        },
        model_id: if fallback {
            "responder-fallback".into()
        } else {
            "responder".into()
        },
        model_version_hash: hex::encode(Sha256::digest(b"responder")),
        system_prompt_hash: system_prompt_hash.to_string(),
        tool_calls: tool_records,
        intermediate_beliefs: vec![IntermediateBelief {
            iteration: 0,
            content_summary: format!(
                "{} {} on {} — shadow allowed={} executed={}",
                req.action, req.target, tenant_id, shadow_check.allowed, executed
            ),
            self_reported_confidence: Some(req.calibrated_confidence),
            timestamp: Utc::now(),
        }],
        evidence_citations: vec![],
        total_iterations: u8::from(!fallback),
        validation_retries: 0,
        cross_review: None,
    };

    let verdict = if shadow_check.allowed {
        Verdict::TruePositive
    } else {
        Verdict::NeedsInvestigation
    };

    let mut envelope = AttestationEnvelope {
        envelope_version: "1.1".into(),
        agent_action_id: action_id,
        case_id,
        tenant_id,
        agent_id: agent_id.to_string(),
        execution_path: ExecutionPathKind::Llm,
        verdict,
        evidence: EvidenceBlock::Llm(evidence),
        timing: TimingBlock {
            started_at,
            finished_at,
            total_ms: latency_ms,
        },
        prev_hash: String::new(),
        signature: String::new(),
        signed_at: finished_at,
    };
    log.sign_and_append(signer, &mut envelope).await?;

    Ok(RespondResult {
        action_id,
        case_id,
        action: req.action,
        target: req.target,
        executed,
        shadow_check,
        latency_ms,
        fallback,
    })
}
