//! Integration test for Phase 7 `run_investigator_llm_loop`.
//!
//! Run: `cargo test -p attest-orchestrator --test investigator_loop`

use attest_inference_router::{ChatClient, ChatRequest, ChatResponse, FinishReason, ToolCall, Usage};
use attest_orchestrator::guardrails::EnforcementMode;
use attest_orchestrator::llm_loop::run_investigator_llm_loop;
use attest_orchestrator::mcp_client::McpClient;
use async_trait::async_trait;
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
            .ok_or_else(|| anyhow::anyhow!("no scripted response at {idx}"))
    }
}

fn tool_call(id: &str, name: &str, args: serde_json::Value) -> ChatResponse {
    ChatResponse {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: args,
        }],
        finish_reason: FinishReason::ToolCalls,
        usage: Usage::default(),
        latency: Duration::from_millis(10),
    }
}

fn final_json(json: &str) -> ChatResponse {
    ChatResponse {
        content: format!("```json\n{json}\n```"),
        tool_calls: vec![],
        finish_reason: FinishReason::Stop,
        usage: Usage::default(),
        latency: Duration::from_millis(20),
    }
}

#[tokio::test]
async fn investigator_calls_warm_tier_then_verdict() {
    std::env::set_var("ATTEST_GUARDRAILS", "off");

    let mcp_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/invoke"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": { "rows": [[1]], "row_count": 1, "source": "test" },
            "call_log": {
                "call_id": Uuid::new_v4().to_string(),
                "args_hash": "aa",
                "result_hash": "bb",
                "latency_ms": 12,
                "policy_decision": {"kind": "allow"},
                "timestamp": "2026-04-29T12:00:00Z"
            },
            "error": null
        })))
        .mount(&mcp_server)
        .await;

    let mcp = McpClient::new(mcp_server.uri());

    let client = ScriptedChatClient::new(vec![
        tool_call(
            "warm_1",
            "query_warm_tier",
            json!({"sql": "SELECT 1", "limit": 10}),
        ),
        final_json(r#"{
            "verdict": "true_positive",
            "confidence": 0.91,
            "reasoning": "Warm tier shows activity [evidence:warm_1].",
            "evidence_citations": ["warm_1"]
        }"#),
    ]);

    let prompt = "Investigator system prompt";
    let hash = hex::encode(Sha256::digest(prompt.as_bytes()));

    let out = run_investigator_llm_loop(
        &client,
        &mcp,
        "investigator-v1",
        prompt,
        &hash,
        &json!({"case_class": "credential_stuffing"}),
        "Triager: needs_investigation (test)",
        0.55,
        8,
        Uuid::new_v4(),
        Some(EnforcementMode::Off),
        None,
    )
    .await
    .expect("investigator loop");

    use attest_attestation::Verdict;
    assert_eq!(out.verdict, Verdict::TruePositive);
    assert!(
        out.evidence
            .tool_calls
            .iter()
            .any(|t| t.tool_id == "query_warm_tier"),
        "expected query_warm_tier in {:?}",
        out.evidence.tool_calls
    );
}
