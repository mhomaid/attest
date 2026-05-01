//! Integration test for the LLM loop.
//!
//! Uses a mock `ChatClient` (tool-call round-trip then final answer) and a
//! wiremock MCP gateway to verify the full chat→tool→chat→final sequence.
//!
//! Run via: `cargo test -p attest-orchestrator --test llm_loop`

use async_trait::async_trait;
use attest_attestation::{ClassifierEvidence, EscalationReason};
use attest_inference_router::{
    ChatClient, ChatRequest, ChatResponse, FinishReason, ToolCall, Usage,
};
use attest_orchestrator::agent::{AgentDefinition, ClassifierArtifact, ExecutionPath};
use attest_orchestrator::llm_loop::run_llm_loop;
use attest_orchestrator::mcp_client::McpClient;
use attest_orchestrator::AgentRole;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

// ── Mock ChatClient ───────────────────────────────────────────────────────────

/// A scripted chat client that returns a fixed sequence of responses.
struct ScriptedChatClient {
    responses: Vec<ChatResponse>,
    call_count: Arc<AtomicUsize>,
}

impl ScriptedChatClient {
    fn new(responses: Vec<ChatResponse>) -> Self {
        Self {
            responses,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ChatClient for ScriptedChatClient {
    fn provider(&self) -> &str {
        "mock"
    }
    fn model_id(&self) -> &str {
        "mock-model"
    }

    async fn chat(&self, _req: ChatRequest) -> anyhow::Result<ChatResponse> {
        let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
        self.responses
            .get(idx)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("ScriptedChatClient: no response at index {idx}"))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_agent() -> AgentDefinition {
    AgentDefinition {
        id: "triager-hybrid-v1".into(),
        role: AgentRole::Triager,
        execution: ExecutionPath::Hybrid {
            primary: Box::new(ExecutionPath::Classifier {
                artifact: ClassifierArtifact {
                    model_path: "unused".into(),
                    shap_background_path: None,
                    novelty_mean_path: "unused".into(),
                    novelty_inv_cov_path: "unused".into(),
                    novelty_threshold_path: "unused".into(),
                    model_artifact_hash: "deadbeef".into(),
                    feature_extractor_hash: "deadbeef".into(),
                    escalation_threshold: 0.6,
                    novelty_threshold: 0.7,
                },
            }),
            escalation: Box::new(ExecutionPath::Llm {
                provider: "mock".into(),
                model_id: "mock-model".into(),
                system_prompt_hash: "prompt-hash".into(),
                max_iterations: 8,
            }),
        },
        version_hash: "test-hash".into(),
    }
}

fn make_classifier_draft() -> ClassifierEvidence {
    ClassifierEvidence {
        model_artifact_hash: "deadbeef".into(),
        feature_extractor_hash: "deadbeef".into(),
        input_features: std::collections::HashMap::new(),
        shap_values: std::collections::HashMap::new(),
        raw_prediction: 0.45,
        calibrated_confidence: 0.40,
        novelty_score: 0.80,
    }
}

fn tool_call_response(tool_id: &str, tool_name: &str, args: serde_json::Value) -> ChatResponse {
    ChatResponse {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: tool_id.into(),
            name: tool_name.into(),
            arguments: args,
        }],
        finish_reason: FinishReason::ToolCalls,
        usage: Usage {
            prompt_tokens: 50,
            completion_tokens: 20,
            total_tokens: 70,
        },
        latency: Duration::from_millis(100),
    }
}

fn final_verdict_response(verdict_json: &str) -> ChatResponse {
    ChatResponse {
        content: format!("```json\n{verdict_json}\n```"),
        tool_calls: vec![],
        finish_reason: FinishReason::Stop,
        usage: Usage {
            prompt_tokens: 200,
            completion_tokens: 80,
            total_tokens: 280,
        },
        latency: Duration::from_millis(500),
    }
}

// ── Test 1: single tool call then final verdict ────────────────────────────────

#[tokio::test]
async fn llm_loop_tool_call_then_verdict() {
    let mcp_server = MockServer::start().await;

    // MCP gateway returns a synthetic baseline
    Mock::given(method("POST"))
        .and(path("/invoke"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {
                "principal": "alice@corp.example",
                "usual_login_hour_range": [8, 18],
                "usual_geo": "US-CA"
            },
            "call_log": {
                "call_id": Uuid::new_v4().to_string(),
                "args_hash": "aa",
                "result_hash": "bb",
                "latency_ms": 15,
                "policy_decision": {"kind": "allow"},
                "timestamp": "2026-04-29T00:00:00Z"
            }
        })))
        .mount(&mcp_server)
        .await;

    let mcp = McpClient::new(mcp_server.uri());

    let client = ScriptedChatClient::new(vec![
        // Round 1: request a user baseline
        tool_call_response(
            "call_001",
            "get_user_baseline",
            json!({"principal": "alice@corp.example"}),
        ),
        // Round 2: emit final verdict
        final_verdict_response(
            r#"{
            "verdict": "benign",
            "confidence": 0.88,
            "reasoning": "User baseline [evidence:call_001] matches expected pattern.",
            "evidence_citations": ["call_001"]
        }"#,
        ),
    ]);

    let agent = make_agent();
    let prompt = "You are a triager.";
    let prompt_hash = hex::encode(Sha256::digest(prompt.as_bytes()));

    let result = run_llm_loop(
        &client,
        &mcp,
        &agent,
        prompt,
        &prompt_hash,
        &json!({"severity_score": 0.3, "actor": "alice"}),
        make_classifier_draft(),
        EscalationReason::HighNoveltyScore {
            score: 0.80,
            threshold: 0.70,
        },
        8,
        Uuid::new_v4(),
        None,
        None,
        None,
    )
    .await
    .expect("LLM loop should succeed");

    use attest_attestation::Verdict;
    assert_eq!(result.verdict, Verdict::Benign);
    assert_eq!(result.evidence.llm_final.total_iterations, 2);
    assert_eq!(result.evidence.llm_final.tool_calls.len(), 1);
    assert_eq!(
        result.evidence.llm_final.tool_calls[0].tool_id,
        "get_user_baseline"
    );
    assert_eq!(
        result.evidence.llm_final.evidence_citations,
        vec!["call_001"]
    );
    assert_eq!(client.call_count(), 2);
}

// ── Test 2: max_iterations exceeded ───────────────────────────────────────────

#[tokio::test]
async fn llm_loop_max_iterations_exceeded() {
    let mcp_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {"data": "ok"},
            "call_log": {
                "call_id": Uuid::new_v4().to_string(),
                "args_hash": "aa",
                "result_hash": "bb",
                "latency_ms": 5,
                "policy_decision": {"kind": "allow"},
                "timestamp": "2026-04-29T00:00:00Z"
            }
        })))
        .expect(3) // 3 tool calls expected
        .mount(&mcp_server)
        .await;

    let mcp = McpClient::new(mcp_server.uri());

    // Always return tool calls — never a final verdict
    let responses: Vec<ChatResponse> = (0..3)
        .map(|i| {
            tool_call_response(
                &format!("call_{i:03}"),
                "get_asset_context",
                json!({"asset_id": "server-01"}),
            )
        })
        .collect();

    let client = ScriptedChatClient::new(responses);
    let agent = make_agent();
    let prompt = "System prompt.";
    let prompt_hash = hex::encode(Sha256::digest(prompt.as_bytes()));

    let err = run_llm_loop(
        &client,
        &mcp,
        &agent,
        prompt,
        &prompt_hash,
        &json!({}),
        make_classifier_draft(),
        EscalationReason::BothLowConfidenceAndHighNovelty,
        3, // max 3 iterations
        Uuid::new_v4(),
        None,
        None,
        None,
    )
    .await
    .expect_err("should fail with MaxIterations");

    use attest_orchestrator::llm_loop::LlmLoopError;
    assert!(matches!(err, LlmLoopError::MaxIterations(3)));
}

// ── Test 3: multiple tool calls in one round ──────────────────────────────────

#[tokio::test]
async fn llm_loop_multi_tool_then_verdict() {
    let mcp_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": {"hit": true, "severity": "high"},
            "call_log": {
                "call_id": Uuid::new_v4().to_string(),
                "args_hash": "cc",
                "result_hash": "dd",
                "latency_ms": 20,
                "policy_decision": {"kind": "allow"},
                "timestamp": "2026-04-29T00:00:00Z"
            }
        })))
        .expect(2) // 2 tool calls in round 1
        .mount(&mcp_server)
        .await;

    let mcp = McpClient::new(mcp_server.uri());

    let round1 = ChatResponse {
        content: String::new(),
        tool_calls: vec![
            ToolCall {
                id: "t1".into(),
                name: "lookup_threat_intel".into(),
                arguments: json!({"indicator": "1.2.3.4", "indicator_type": "ip"}),
            },
            ToolCall {
                id: "t2".into(),
                name: "get_user_baseline".into(),
                arguments: json!({"principal": "bob@corp"}),
            },
        ],
        finish_reason: FinishReason::ToolCalls,
        usage: Usage::default(),
        latency: Duration::from_millis(200),
    };

    let client = ScriptedChatClient::new(vec![
        round1,
        final_verdict_response(
            r#"{"verdict": "true_positive", "confidence": 0.93, "reasoning": "Threat intel hit [evidence:t1] plus anomalous baseline [evidence:t2].", "evidence_citations": ["t1", "t2"]}"#,
        ),
    ]);

    let agent = make_agent();
    let prompt = "Triager system prompt.";
    let prompt_hash = hex::encode(Sha256::digest(prompt.as_bytes()));

    let result = run_llm_loop(
        &client,
        &mcp,
        &agent,
        prompt,
        &prompt_hash,
        &json!({"ip": "1.2.3.4"}),
        make_classifier_draft(),
        EscalationReason::LowCalibratedConfidence {
            score: 0.40,
            threshold: 0.60,
        },
        8,
        Uuid::new_v4(),
        None,
        None,
        None,
    )
    .await
    .expect("should succeed");

    use attest_attestation::Verdict;
    assert_eq!(result.verdict, Verdict::TruePositive);
    assert_eq!(result.evidence.llm_final.tool_calls.len(), 2);
    assert_eq!(
        result.evidence.llm_final.evidence_citations,
        vec!["t1", "t2"]
    );
    assert_eq!(result.evidence.llm_final.total_iterations, 2);
}
