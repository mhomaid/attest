//! Integration tests for Phase 5 guardrails — verifies the guardrail checks
//! fire correctly when wired inside `run_llm_loop`.
//!
//! Uses a `ScriptedChatClient` (no real LLM) and `wiremock` for the MCP gateway
//! so these tests run without any external service.

use anyhow::Result;
use async_trait::async_trait;
use attest_attestation::{EscalationReason, Verdict};
use attest_inference_router::{
    ChatClient, ChatRequest, ChatResponse, FinishReason, ToolCall, Usage,
};
use attest_orchestrator::{
    agent::{AgentDefinition, AgentRole, ClassifierArtifact, ExecutionPath},
    guardrails::EnforcementMode,
    llm_loop::run_llm_loop,
    mcp_client::McpClient,
};
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;
use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

// ── ScriptedChatClient ────────────────────────────────────────────────────────

/// A deterministic ChatClient that pops pre-programmed responses from a queue.
struct ScriptedChatClient {
    responses: Arc<Mutex<VecDeque<ChatResponse>>>,
    provider: &'static str,
    model_id: &'static str,
}

impl ScriptedChatClient {
    fn new(responses: Vec<ChatResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
            provider: "scripted",
            model_id: "scripted-model",
        }
    }
}

#[async_trait]
impl ChatClient for ScriptedChatClient {
    async fn chat(&self, _req: ChatRequest) -> Result<ChatResponse> {
        let mut q = self.responses.lock().unwrap();
        q.pop_front().ok_or_else(|| anyhow::anyhow!("ScriptedChatClient: no more responses"))
    }
    fn provider(&self) -> &str { self.provider }
    fn model_id(&self) -> &str { self.model_id }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn plain_response(content: impl Into<String>) -> ChatResponse {
    ChatResponse {
        content: content.into(),
        tool_calls: vec![],
        finish_reason: FinishReason::Stop,
        usage: Usage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 },
        latency: Duration::from_millis(10),
    }
}

fn tool_call_response(id: &str, name: &str, args: serde_json::Value) -> ChatResponse {
    ChatResponse {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: args,
        }],
        finish_reason: FinishReason::ToolCalls,
        usage: Usage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 },
        latency: Duration::from_millis(10),
    }
}

fn verdict_json(verdict: &str, reasoning: &str, citations: &[&str]) -> String {
    let cits: Vec<String> = citations.iter().map(|s| s.to_string()).collect();
    format!(
        r#"{{"verdict":"{verdict}","confidence":0.9,"reasoning":"{reasoning}","evidence_citations":{}}}"#,
        serde_json::to_string(&cits).unwrap()
    )
}

fn minimal_classifier_evidence() -> attest_attestation::ClassifierEvidence {
    attest_attestation::ClassifierEvidence {
        model_artifact_hash: "test".into(),
        feature_extractor_hash: "test".into(),
        input_features: Default::default(),
        shap_values: Default::default(),
        raw_prediction: 0.3,
        calibrated_confidence: 0.3,
        novelty_score: 0.8,
    }
}

fn mock_agent() -> AgentDefinition {
    AgentDefinition {
        id: "test-triager".into(),
        role: AgentRole::Triager,
        execution: ExecutionPath::Hybrid {
            primary: Box::new(ExecutionPath::Classifier {
                artifact: ClassifierArtifact {
                    model_path: "/tmp/model.onnx".into(),
                    shap_background_path: None,
                    novelty_mean_path: "/tmp/mean.npy".into(),
                    novelty_inv_cov_path: "/tmp/inv_cov.npy".into(),
                    novelty_threshold_path: "/tmp/novelty_threshold.txt".into(),
                    model_artifact_hash: "test".into(),
                    feature_extractor_hash: "test".into(),
                    escalation_threshold: 0.6,
                    novelty_threshold: 0.7,
                },
            }),
            escalation: Box::new(ExecutionPath::Llm {
                provider: "scripted".into(),
                model_id: "scripted-model".into(),
                system_prompt_hash: "abc123".into(),
                max_iterations: 10,
            }),
        },
        version_hash: "test".into(),
    }
}

async fn start_mcp_mock() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {"data": []},
            "error": null,
            "call_log": {
                "call_id": "00000000-0000-0000-0000-000000000000",
                "args_hash": "aa",
                "result_hash": "bb",
                "latency_ms": 5,
                "policy_decision": {"kind": "allow"},
                "timestamp": "2025-01-01T00:00:00Z"
            }
        })))
        .mount(&server)
        .await;
    server
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Guardrail 1: A verdict with no tool calls should be re-prompted and then —
/// when the LLM still doesn't call any tools — eventually force `NeedsInvestigation`
/// after exhausting `GUARDRAIL_MAX_RETRIES`.
#[tokio::test]
async fn guardrail_no_retrieval_forces_needs_investigation() {
    let mcp_server = start_mcp_mock().await;
    let mcp = McpClient::new(mcp_server.uri());

    // Cap = 2 means: 2 re-prompts allowed, 3rd attempt hits cap → NeedsInvestigation.
    // Responses: iter0 verdict (no tools) → retry1 | iter1 verdict → retry2 | iter2 verdict → cap → done
    let responses = vec![
        plain_response(verdict_json("true_positive", "Login anomaly.", &[])),
        plain_response(verdict_json("true_positive", "Login anomaly.", &[])),
        plain_response(verdict_json("true_positive", "Login anomaly.", &[])),
    ];

    let client = ScriptedChatClient::new(responses);
    let result = run_llm_loop(
        &client,
        &mcp,
        &mock_agent(),
        "You are a triager.",
        "prompt_hash_abc",
        &json!({"event_type": "login", "principal": "alice"}),
        minimal_classifier_evidence(),
        EscalationReason::BothLowConfidenceAndHighNovelty,
        10,
        Uuid::new_v4(),
        Some(EnforcementMode::On),
        Some(2),
    ).await.expect("loop should not error — should produce NeedsInvestigation");

    assert_eq!(
        result.verdict,
        Verdict::NeedsInvestigation,
        "should degrade to NeedsInvestigation after no-retrieval retries exhausted"
    );
    assert!(
        result.evidence.llm_final.validation_retries >= 2,
        "validation_retries should be recorded"
    );
}

/// Guardrail 2: A verdict that cites a fabricated tool call ID should be
/// re-prompted; after retries are exhausted, `NeedsInvestigation` is returned.
#[tokio::test]
async fn guardrail_orphan_citation_forces_needs_investigation() {
    let mcp_server = start_mcp_mock().await;
    let mcp = McpClient::new(mcp_server.uri());

    // First: tool call (allowed, so retrieval check passes)
    // Then: verdict with fabricated citation (3 times to exhaust retries with cap=2)
    let responses = vec![
        tool_call_response("real_call_001", "get_user_baseline", json!({"principal": "alice"})),
        plain_response(verdict_json(
            "true_positive",
            "IP was malicious [evidence:invented_id_xyz].",
            &["invented_id_xyz"],
        )),
        plain_response(verdict_json(
            "true_positive",
            "IP was malicious [evidence:invented_id_xyz].",
            &["invented_id_xyz"],
        )),
        plain_response(verdict_json(
            "true_positive",
            "IP was malicious [evidence:invented_id_xyz].",
            &["invented_id_xyz"],
        )),
    ];

    let client = ScriptedChatClient::new(responses);
    let result = run_llm_loop(
        &client,
        &mcp,
        &mock_agent(),
        "You are a triager.",
        "prompt_hash_abc",
        &json!({"event_type": "login", "principal": "alice"}),
        minimal_classifier_evidence(),
        EscalationReason::BothLowConfidenceAndHighNovelty,
        10,
        Uuid::new_v4(),
        Some(EnforcementMode::On),
        Some(2),
    ).await.expect("should not hard-error");

    assert_eq!(
        result.verdict,
        Verdict::NeedsInvestigation,
        "orphan citation should degrade verdict"
    );
}

/// Guardrail happy path: tool call followed by a properly-cited verdict passes.
#[tokio::test]
async fn guardrail_passes_when_tool_called_and_citation_real() {
    let mcp_server = start_mcp_mock().await;
    let mcp = McpClient::new(mcp_server.uri());

    let responses = vec![
        // LLM calls a tool first
        tool_call_response("call_001", "get_user_baseline", json!({"principal": "alice"})),
        // Then emits a verdict citing the real tool call ID
        plain_response(verdict_json(
            "true_positive",
            "The user principal showed anomalous login [evidence:call_001].",
            &["call_001"],
        )),
    ];

    let client = ScriptedChatClient::new(responses);
    let result = run_llm_loop(
        &client,
        &mcp,
        &mock_agent(),
        "You are a triager.",
        "prompt_hash_abc",
        &json!({"event_type": "login", "principal": "alice"}),
        minimal_classifier_evidence(),
        EscalationReason::BothLowConfidenceAndHighNovelty,
        10,
        Uuid::new_v4(),
        Some(EnforcementMode::On),
        Some(3),
    ).await.expect("should complete successfully");

    assert_eq!(result.verdict, Verdict::TruePositive);
    assert_eq!(result.evidence.llm_final.validation_retries, 0);
}

/// Guardrail retry-then-pass: first verdict has no tool calls, re-prompt causes
/// the LLM to call a tool and then emit a proper verdict.
#[tokio::test]
async fn guardrail_retry_then_pass() {
    let mcp_server = start_mcp_mock().await;
    let mcp = McpClient::new(mcp_server.uri());

    let responses = vec![
        // First response: jumps straight to verdict (no tool call) — rejected
        plain_response(verdict_json("true_positive", "Suspicious login.", &[])),
        // Re-prompted: now calls tool
        tool_call_response("call_002", "lookup_threat_intel", json!({"indicator": "1.2.3.4", "indicator_type": "ip"})),
        // Then emits proper verdict
        plain_response(verdict_json(
            "true_positive",
            "IP 1.2.3.4 is listed in threat intel [evidence:call_002].",
            &["call_002"],
        )),
    ];

    let client = ScriptedChatClient::new(responses);
    let result = run_llm_loop(
        &client,
        &mcp,
        &mock_agent(),
        "You are a triager.",
        "prompt_hash_abc",
        &json!({"event_type": "login", "src_ip": "1.2.3.4"}),
        minimal_classifier_evidence(),
        EscalationReason::BothLowConfidenceAndHighNovelty,
        10,
        Uuid::new_v4(),
        Some(EnforcementMode::On),
        Some(3),
    ).await.expect("should complete successfully after retry");

    assert_eq!(result.verdict, Verdict::TruePositive);
    assert_eq!(result.evidence.llm_final.validation_retries, 1, "should record 1 retry");
}
