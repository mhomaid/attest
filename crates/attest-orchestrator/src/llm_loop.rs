//! Multi-turn LLM agent loop with MCP tool invocations.
//!
//! `run_llm_loop` drives the conversation until the model produces a final
//! JSON verdict or the iteration cap is reached.  Every tool call is routed
//! through the MCP gateway (policy-checked, logged).  The resulting
//! `LlmEvidence` is dropped into `EvidenceBlock::Hybrid` by the caller.
//!
//! Phase 5 adds three guardrail checks in the final-verdict branch:
//!   1. Retrieval-before-reasoning
//!   2. Citation enforcement
//!   3. (Cross-review handled in `triage.rs` via `run_review`)

use crate::agent::{AgentDefinition, ExecutionPath};
use crate::guardrails::{
    citation_reprompt, has_retrieved_evidence, max_validation_retries,
    retrieval_reprompt, validate_citations, EnforcementMode,
};
use crate::mcp_client::McpClient;
use anyhow::Result;
use attest_attestation::{
    CrossReviewBlock, EscalationReason, HybridEvidence, IntermediateBelief, LlmEvidence,
    ToolCallRecord, ClassifierEvidence,
};
use attest_inference_router::{ChatClient, ChatMessage, ChatRequest, ToolDef};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Errors specific to the LLM loop (separate from infra errors).
#[derive(Debug, thiserror::Error)]
pub enum LlmLoopError {
    #[error("LLM provider error: {0}")]
    Provider(#[from] anyhow::Error),

    #[error("exceeded max iterations ({0}) without a final verdict")]
    MaxIterations(u8),

    #[error("LLM returned an invalid verdict JSON: {0}")]
    InvalidVerdict(String),
}

/// A verdict parsed from the model's final JSON output.
#[derive(Debug, Deserialize)]
struct ModelVerdict {
    /// One of: true_positive, false_positive, benign, needs_investigation
    verdict: String,
    /// 0.0–1.0 self-reported confidence.
    confidence: Option<f32>,
    /// Free-form reasoning text.
    #[serde(default)]
    reasoning: String,
    /// `[evidence:ocsf_event_id]` tags extracted from the text.
    #[serde(default)]
    evidence_citations: Vec<String>,
}

/// Tool definitions exposed to the triager agent.
/// Mirrors the 4 read-only tools in `attest-mcp-gateway/src/registry.rs`
/// (excludes `query_warm_tier` which is reserved for Phase 7 Investigator).
fn triager_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "query_hot_tier".into(),
            description: "Query ClickHouse hot-tier for recent events associated with a principal or IP address.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "filter": {
                        "type": "string",
                        "description": "SQL WHERE clause fragment, e.g. actor_user_name='alice' AND time > now() - INTERVAL 1 HOUR"
                    },
                    "limit": { "type": "integer", "default": 20 }
                },
                "required": ["filter"]
            }),
        },
        ToolDef {
            name: "lookup_threat_intel".into(),
            description: "Check an IP address or domain against threat-intelligence feeds.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "indicator": { "type": "string", "description": "IP, domain, or hash to look up" },
                    "indicator_type": {
                        "type": "string",
                        "enum": ["ip", "domain", "sha256"],
                        "description": "Type of the indicator"
                    }
                },
                "required": ["indicator", "indicator_type"]
            }),
        },
        ToolDef {
            name: "get_asset_context".into(),
            description: "Retrieve asset metadata (owner, criticality, location) for a given asset ID or hostname.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "asset_id": { "type": "string" }
                },
                "required": ["asset_id"]
            }),
        },
        ToolDef {
            name: "get_user_baseline".into(),
            description: "Fetch historical behavioural baseline for a user principal (login times, typical geo, usual APIs called).".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "principal": {
                        "type": "string",
                        "description": "User principal (e.g. user@example.com or IAM ARN)"
                    }
                },
                "required": ["principal"]
            }),
        },
    ]
}

/// Build `LlmEvidence` metadata from loop state.
fn build_llm_evidence(
    agent: &AgentDefinition,
    system_prompt_hash: &str,
    tool_calls: Vec<ToolCallRecord>,
    intermediate_beliefs: Vec<IntermediateBelief>,
    evidence_citations: Vec<String>,
    total_iterations: u8,
    validation_retries: u8,
) -> LlmEvidence {
    let (provider, model_id) = match &agent.execution {
        ExecutionPath::Hybrid { escalation, .. } => match escalation.as_ref() {
            ExecutionPath::Llm { provider, model_id, .. } => (provider.clone(), model_id.clone()),
            _ => ("unknown".into(), "unknown".into()),
        },
        ExecutionPath::Llm { provider, model_id, .. } => (provider.clone(), model_id.clone()),
        _ => ("unknown".into(), "unknown".into()),
    };

    // Derive a model version hash from the model ID string
    let model_version_hash = hex::encode(Sha256::digest(model_id.as_bytes()));

    LlmEvidence {
        model_provider: provider,
        model_id,
        model_version_hash,
        system_prompt_hash: system_prompt_hash.to_string(),
        tool_calls,
        intermediate_beliefs,
        evidence_citations,
        total_iterations,
        validation_retries,
        cross_review: None,
    }
}

/// Map a model verdict string to the attestation `Verdict` enum.
fn parse_verdict(s: &str) -> attest_attestation::Verdict {
    use attest_attestation::Verdict;
    match s.to_ascii_lowercase().as_str() {
        "true_positive" | "true positive" => Verdict::TruePositive,
        "false_positive" | "false positive" => Verdict::FalsePositive,
        "benign" => Verdict::Benign,
        "needs_investigation" | "needs investigation" => Verdict::NeedsInvestigation,
        _ => Verdict::NeedsInvestigation,
    }
}

/// The fully-resolved result of running the LLM loop.
#[derive(Debug)]
pub struct LlmLoopResult {
    pub evidence: HybridEvidence,
    pub verdict: attest_attestation::Verdict,
}

/// Outcome of the cross-reviewer pass (see `run_review`).
#[derive(Debug)]
pub struct ReviewOutcome {
    pub agrees: bool,
    pub disagreement_reason: Option<String>,
    pub intermediate_beliefs: Vec<IntermediateBelief>,
}

/// Run the multi-turn LLM agent loop.
///
/// # Arguments
/// * `client` — `ChatClient` implementation (OpenAI-compat or Anthropic).
/// * `mcp` — MCP gateway client for tool invocations.
/// * `agent` — Agent definition (used to derive model metadata for `LlmEvidence`).
/// * `system_prompt` — Full system prompt text.
/// * `system_prompt_hash` — Pre-computed SHA-256 hex of the system prompt.
/// * `alert` — Raw alert JSON (forwarded as the first user message).
/// * `classifier_draft` — Phase 4a classifier evidence to embed in the hybrid envelope.
/// * `escalation_reason` — Why the classifier escalated.
/// * `max_iterations` — Maximum chat rounds before giving up (default 8).
/// * `action_id` — Shared action ID for MCP audit log correlation.
/// * `enforcement` — Guardrail enforcement mode (read from env if not supplied).
/// * `guardrail_retries_cap` — Max per-check re-prompts (read from env if `None`).
#[allow(clippy::too_many_arguments)]
pub async fn run_llm_loop(
    client: &dyn ChatClient,
    mcp: &McpClient,
    agent: &AgentDefinition,
    system_prompt: &str,
    system_prompt_hash: &str,
    alert: &Value,
    classifier_draft: ClassifierEvidence,
    escalation_reason: EscalationReason,
    max_iterations: u8,
    action_id: Uuid,
    enforcement: Option<EnforcementMode>,
    guardrail_retries_cap: Option<u8>,
) -> Result<LlmLoopResult, LlmLoopError> {
    let enforcement = enforcement.unwrap_or_else(EnforcementMode::from_env);
    let guardrail_retries_cap = guardrail_retries_cap.unwrap_or_else(max_validation_retries);
    let tools = triager_tools();
    let mut messages: Vec<ChatMessage> = vec![
        ChatMessage::system(system_prompt),
        ChatMessage::user(format!(
            "Triage the following security alert. The ONNX classifier was uncertain (calibrated confidence: {:.3}, novelty score: {:.3}). \
             Use the available tools to gather evidence, then emit a final JSON verdict.\n\nAlert:\n{}",
            classifier_draft.calibrated_confidence,
            classifier_draft.novelty_score,
            serde_json::to_string_pretty(alert).unwrap_or_else(|_| alert.to_string())
        )),
    ];

    let mut tool_call_records: Vec<ToolCallRecord> = Vec::new();
    let mut intermediate_beliefs: Vec<IntermediateBelief> = Vec::new();
    let mut validation_retries: u8 = 0;
    // IDs of tool calls (from the LLM response) that MCP allowed successfully.
    let mut successful_call_ids: Vec<String> = Vec::new();

    for iteration in 0..max_iterations {
        let req = ChatRequest::new(messages.clone(), tools.clone());
        let resp = client.chat(req).await.map_err(LlmLoopError::Provider)?;

        tracing::debug!(
            iteration,
            action_id = %action_id,
            has_tool_calls = resp.has_tool_calls(),
            content_len = resp.content.len(),
            latency_ms = resp.latency.as_millis(),
            "LLM loop iteration"
        );

        if resp.has_tool_calls() {
            // Record an intermediate belief before executing tool calls
            intermediate_beliefs.push(IntermediateBelief {
                iteration,
                content_summary: if resp.content.is_empty() {
                    format!("requesting {} tool calls", resp.tool_calls.len())
                } else {
                    truncate_summary(&resp.content, 200)
                },
                self_reported_confidence: None,
                timestamp: Utc::now(),
            });

            // Append assistant message with tool calls
            messages.push(ChatMessage::Assistant {
                content: resp.content.clone(),
                tool_calls: Some(resp.tool_calls.iter().map(|tc| {
                    attest_inference_router::ToolCall {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                    }
                }).collect()),
            });

            // Execute each tool call through the MCP gateway
            for tc in &resp.tool_calls {
                let result = mcp.invoke(
                    &agent.id,
                    action_id,
                    "triager",
                    &tc.name,
                    &tc.arguments,
                    classifier_draft.calibrated_confidence,
                ).await;

                let (tool_output, record) = match result {
                    Ok(r) => {
                        let output = r.result
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| r.error.unwrap_or_else(|| "tool denied".into()));
                        (output, r.record)
                    }
                    Err(e) => {
                        tracing::warn!(
                            tool = %tc.name,
                            error = %e,
                            "MCP tool call failed — returning error to LLM"
                        );
                        let dummy_record = ToolCallRecord {
                            tool_id: tc.name.clone(),
                            args_hash: hex::encode(Sha256::digest(
                                serde_json::to_string(&tc.arguments).unwrap_or_default()
                            )),
                            result_hash: hex::encode(Sha256::digest(e.to_string().as_bytes())),
                            latency_ms: 0,
                            policy_decision: "error".into(),
                            timestamp: Utc::now(),
                        };
                        (format!("error: {e}"), dummy_record)
                    }
                };

                tool_call_records.push(record);
                // Track this call ID if the MCP allowed the call
                if tool_call_records.last().map(|r| r.policy_decision == "allow").unwrap_or(false) {
                    successful_call_ids.push(tc.id.clone());
                }
                messages.push(ChatMessage::tool_result(tc.id.clone(), tool_output));
            }
        } else {
            // No tool calls — this is the final verdict response
            let raw_content = resp.content.trim().to_string();

            tracing::debug!(
                iteration,
                action_id = %action_id,
                raw_len = raw_content.len(),
                raw_preview = %raw_content.chars().take(200).collect::<String>(),
                "LLM final response"
            );

            // Parse JSON verdict — try to extract from markdown fence if needed
            let json_str = extract_json_block(&raw_content);
            let model_verdict: ModelVerdict = serde_json::from_str(&json_str)
                .map_err(|e| LlmLoopError::InvalidVerdict(
                    format!("could not parse verdict JSON: {e}\nraw: {json_str}")
                ))?;

            // ── Phase 5 Guardrail 1: Retrieval-before-reasoning ──────────────
            if enforcement == EnforcementMode::On
                && !has_retrieved_evidence(&tool_call_records)
            {
                if validation_retries >= guardrail_retries_cap {
                    tracing::warn!(
                        action_id = %action_id,
                        validation_retries,
                        "guardrail: no retrieval after max retries — forcing NeedsInvestigation"
                    );
                    intermediate_beliefs.push(IntermediateBelief {
                        iteration,
                        content_summary: "guardrail: no retrieval — capped".into(),
                        self_reported_confidence: Some(0.0),
                        timestamp: Utc::now(),
                    });
                    let llm_evidence = build_llm_evidence(
                        agent, system_prompt_hash, tool_call_records,
                        intermediate_beliefs, vec![], iteration + 1, validation_retries,
                    );
                    let hybrid = HybridEvidence { classifier_draft, llm_final: llm_evidence, escalation_reason };
                    return Ok(LlmLoopResult {
                        evidence: hybrid,
                        verdict: attest_attestation::Verdict::NeedsInvestigation,
                    });
                }

                validation_retries += 1;
                tracing::warn!(
                    action_id = %action_id,
                    validation_retries,
                    "guardrail: no retrieval — re-prompting"
                );
                intermediate_beliefs.push(IntermediateBelief {
                    iteration,
                    content_summary: format!("guardrail_retry:{validation_retries} no-retrieval"),
                    self_reported_confidence: Some(0.0),
                    timestamp: Utc::now(),
                });
                messages.push(ChatMessage::user(retrieval_reprompt()));
                continue;
            }

            // ── Phase 5 Guardrail 2: Citation enforcement ────────────────────
            let citation_report = validate_citations(
                &model_verdict.reasoning,
                &model_verdict.evidence_citations,
                &successful_call_ids,
            );

            if enforcement == EnforcementMode::On && !citation_report.passed {
                if validation_retries >= guardrail_retries_cap {
                    tracing::warn!(
                        action_id = %action_id,
                        validation_retries,
                        orphans = ?citation_report.orphan_citations,
                        "guardrail: citation failure after max retries — forcing NeedsInvestigation"
                    );
                    intermediate_beliefs.push(IntermediateBelief {
                        iteration,
                        content_summary: "guardrail: citation failure — capped".into(),
                        self_reported_confidence: Some(0.0),
                        timestamp: Utc::now(),
                    });
                    let llm_evidence = build_llm_evidence(
                        agent, system_prompt_hash, tool_call_records,
                        intermediate_beliefs, model_verdict.evidence_citations,
                        iteration + 1, validation_retries,
                    );
                    let hybrid = HybridEvidence { classifier_draft, llm_final: llm_evidence, escalation_reason };
                    return Ok(LlmLoopResult {
                        evidence: hybrid,
                        verdict: attest_attestation::Verdict::NeedsInvestigation,
                    });
                }

                validation_retries += 1;
                tracing::warn!(
                    action_id = %action_id,
                    validation_retries,
                    orphans = ?citation_report.orphan_citations,
                    "guardrail: citation failure — re-prompting"
                );
                intermediate_beliefs.push(IntermediateBelief {
                    iteration,
                    content_summary: format!("guardrail_retry:{validation_retries} citation-fail"),
                    self_reported_confidence: Some(0.0),
                    timestamp: Utc::now(),
                });
                messages.push(ChatMessage::user(citation_reprompt(&citation_report)));
                continue;
            }

            // ── Guardrails passed — record final belief and return ────────────
            intermediate_beliefs.push(IntermediateBelief {
                iteration,
                content_summary: if model_verdict.reasoning.is_empty() {
                    truncate_summary(&raw_content, 300)
                } else {
                    truncate_summary(&model_verdict.reasoning, 300)
                },
                self_reported_confidence: model_verdict.confidence,
                timestamp: Utc::now(),
            });

            let verdict = parse_verdict(&model_verdict.verdict);
            let evidence_citations = model_verdict.evidence_citations;
            let total_iterations = iteration + 1;

            let llm_evidence = build_llm_evidence(
                agent,
                system_prompt_hash,
                tool_call_records,
                intermediate_beliefs,
                evidence_citations,
                total_iterations,
                validation_retries,
            );

            let hybrid = HybridEvidence {
                classifier_draft,
                llm_final: llm_evidence,
                escalation_reason,
            };

            return Ok(LlmLoopResult { evidence: hybrid, verdict });
        }
    }

    Err(LlmLoopError::MaxIterations(max_iterations))
}

/// Run a single-round reviewer pass for cross-agent review.
///
/// Sends the primary verdict to a second LLM invocation (no tools, low
/// temperature) and asks it to agree or disagree.  Returns a `ReviewOutcome`
/// that the caller embeds in `CrossReviewBlock`.
pub async fn run_review(
    client: &dyn ChatClient,
    reviewer_prompt: &str,
    reviewer_prompt_hash: &str,
    verdict: &str,
    reasoning: &str,
    alert: &Value,
) -> ReviewOutcome {
    let user_msg = format!(
        "You are reviewing a security triage verdict made by another analyst.\n\n\
         Alert:\n{}\n\n\
         Primary verdict: **{verdict}**\n\
         Primary reasoning: {reasoning}\n\n\
         Do you agree with this verdict? Respond ONLY with a JSON object:\n\
         {{\"agrees\": true/false, \"reason\": \"brief explanation\"}}",
        serde_json::to_string_pretty(alert).unwrap_or_else(|_| alert.to_string()),
    );

    let messages = vec![
        ChatMessage::system(reviewer_prompt),
        ChatMessage::user(user_msg),
    ];
    // No tools; single round; low temperature enforced by system prompt instruction.
    let req = ChatRequest::new(messages, vec![]);

    let reviewer_beliefs: Vec<IntermediateBelief> = Vec::new();

    match client.chat(req).await {
        Ok(resp) => {
            let raw = extract_json_block(resp.content.trim());
            #[derive(Deserialize)]
            struct ReviewJson { agrees: bool, #[serde(default)] reason: String }
            match serde_json::from_str::<ReviewJson>(&raw) {
                Ok(r) => ReviewOutcome {
                    agrees: r.agrees,
                    disagreement_reason: if r.agrees { None } else { Some(r.reason) },
                    intermediate_beliefs: reviewer_beliefs,
                },
                Err(e) => {
                    tracing::warn!(
                        reviewer_prompt_hash,
                        error = %e,
                        raw_preview = %raw.chars().take(200).collect::<String>(),
                        "reviewer: could not parse response — treating as disagree"
                    );
                    ReviewOutcome {
                        agrees: false,
                        disagreement_reason: Some(format!("parse error: {e}")),
                        intermediate_beliefs: reviewer_beliefs,
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!(reviewer_prompt_hash, error = %e, "reviewer: LLM call failed");
            ReviewOutcome {
                agrees: false,
                disagreement_reason: Some(format!("reviewer call failed: {e}")),
                intermediate_beliefs: reviewer_beliefs,
            }
        }
    }
}

/// Build a `CrossReviewBlock` from a `ReviewOutcome`.
pub fn build_cross_review_block(
    reviewer_prompt_hash: &str,
    outcome: ReviewOutcome,
) -> CrossReviewBlock {
    CrossReviewBlock {
        reviewer_prompt_hash: reviewer_prompt_hash.to_string(),
        agrees: outcome.agrees,
        disagreement_reason: outcome.disagreement_reason,
        reviewer_intermediate_beliefs: outcome.intermediate_beliefs,
    }
}

/// Extract the first JSON object from a string.
///
/// Handles:
/// - Qwen3 `<think>...</think>` reasoning blocks (stripped first)
/// - Markdown ` ```json ... ``` ` fences
/// - Plain ` ``` ... ``` ` fences
/// - Bare `{ ... }` objects
fn extract_json_block(text: &str) -> String {
    // 1. Strip Qwen3-style <think>…</think> blocks before looking for JSON
    let stripped = if let (Some(open), Some(close)) = (text.find("<think>"), text.rfind("</think>")) {
        if close > open {
            let before = &text[..open];
            let after = &text[close + "</think>".len()..];
            format!("{before}{after}")
        } else {
            text.to_string()
        }
    } else {
        text.to_string()
    };
    let text = stripped.trim();

    // 2. ```json … ``` fence
    if let Some(start) = text.find("```json") {
        let rest = &text[start + 7..];
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_string();
        }
    }

    // 3. Plain ``` … ``` fence
    if let Some(start) = text.find("```") {
        let rest = &text[start + 3..];
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_string();
        }
    }

    // 4. Bare JSON object — find outermost { … }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                return text[start..=end].to_string();
            }
        }
    }

    text.to_string()
}

fn truncate_summary(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_from_fence() {
        let text = "Let me analyze this.\n```json\n{\"verdict\": \"benign\", \"confidence\": 0.9}\n```\n";
        let extracted = extract_json_block(text);
        assert!(extracted.contains("benign"));
    }

    #[test]
    fn extract_json_bare() {
        let text = "Based on my analysis: {\"verdict\": \"true_positive\", \"confidence\": 0.85}";
        let extracted = extract_json_block(text);
        let v: Value = serde_json::from_str(&extracted).unwrap();
        assert_eq!(v["verdict"], "true_positive");
    }

    #[test]
    fn parse_verdict_variants() {
        use attest_attestation::Verdict;
        assert_eq!(parse_verdict("true_positive"), Verdict::TruePositive);
        assert_eq!(parse_verdict("false_positive"), Verdict::FalsePositive);
        assert_eq!(parse_verdict("benign"), Verdict::Benign);
        assert_eq!(parse_verdict("needs_investigation"), Verdict::NeedsInvestigation);
        assert_eq!(parse_verdict("unknown_garbage"), Verdict::NeedsInvestigation);
    }

    #[test]
    fn extract_json_strips_think_blocks() {
        let text = "<think>\nI need to analyze this carefully. The alert is OOD.\n</think>\n```json\n{\"verdict\": \"benign\", \"confidence\": 0.82, \"reasoning\": \"ok\", \"evidence_citations\": []}\n```";
        let extracted = extract_json_block(text);
        let v: Value = serde_json::from_str(&extracted).unwrap();
        assert_eq!(v["verdict"], "benign");
    }

    #[test]
    fn extract_json_bare_after_think() {
        let text = "<think>reasoning here</think>\nFinal answer: {\"verdict\": \"true_positive\", \"confidence\": 0.9, \"reasoning\": \"threat confirmed\", \"evidence_citations\": []}";
        let extracted = extract_json_block(text);
        let v: Value = serde_json::from_str(&extracted).unwrap();
        assert_eq!(v["verdict"], "true_positive");
    }
}
