//! HTTP server for the MCP gateway.
//!
//! Endpoint: `POST /invoke`
//! Body: `{ "agent_id": "...", "action_id": "...", "agent_role": "...", "tool_id": "...", "args": {...} }`
//! Response: `{ "result": {...}, "call_log": {...} }`

use crate::registry::ToolRegistry;
use crate::tools;
use attest_policy_engine::{authorize, AgentRole, PolicyContext, PolicyDecision};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::post,
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone)]
pub struct GatewayState {
    pub registry: Arc<ToolRegistry>,
}

#[derive(Debug, Deserialize)]
pub struct InvokeRequest {
    pub agent_id: String,
    pub action_id: Uuid,
    pub agent_role: AgentRole,
    pub tool_id: String,
    pub args: Value,
    /// Optional calibrated confidence passed from the orchestrator for policy checks.
    #[serde(default)]
    pub calibrated_confidence: f32,
}

#[derive(Debug, Serialize)]
pub struct ToolCallLog {
    pub call_id: Uuid,
    pub agent_id: String,
    pub action_id: Uuid,
    pub tool_id: String,
    pub args_hash: String,
    pub result_hash: String,
    pub latency_ms: u64,
    pub policy_decision: PolicyDecision,
    pub timestamp: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InvokeResponse {
    pub result: Option<Value>,
    pub call_log: ToolCallLog,
    pub error: Option<String>,
}

pub fn build_router(registry: ToolRegistry) -> Router {
    let state = GatewayState { registry: Arc::new(registry) };
    Router::new()
        .route("/invoke", post(handle_invoke))
        .route("/tools", axum::routing::get(handle_list_tools))
        .with_state(state)
}

async fn handle_invoke(
    State(state): State<GatewayState>,
    Json(req): Json<InvokeRequest>,
) -> impl IntoResponse {
    let timestamp = Utc::now();
    let args_hash = hex::encode(Sha256::digest(serde_json::to_string(&req.args).unwrap_or_default()));

    // 1. Policy check
    let policy_ctx = PolicyContext {
        calibrated_confidence: req.calibrated_confidence,
        ..Default::default()
    };
    let policy_decision = authorize(&req.agent_role, &req.tool_id, &policy_ctx);

    if !policy_decision.is_allowed() {
        let log = ToolCallLog {
            call_id: Uuid::new_v4(),
            agent_id: req.agent_id.clone(),
            action_id: req.action_id,
            tool_id: req.tool_id.clone(),
            args_hash,
            result_hash: String::new(),
            latency_ms: 0,
            policy_decision: policy_decision.clone(),
            timestamp,
        };
        tracing::warn!(
            agent_id = %req.agent_id,
            tool_id = %req.tool_id,
            decision = ?policy_decision,
            "tool call denied by policy engine"
        );
        let resp = InvokeResponse { result: None, call_log: log, error: Some(format!("{:?}", policy_decision)) };
        return (StatusCode::FORBIDDEN, Json(resp));
    }

    // 2. Tool must be registered
    if state.registry.get(&req.tool_id).is_none() {
        let log = ToolCallLog {
            call_id: Uuid::new_v4(),
            agent_id: req.agent_id.clone(),
            action_id: req.action_id,
            tool_id: req.tool_id.clone(),
            args_hash,
            result_hash: String::new(),
            latency_ms: 0,
            policy_decision,
            timestamp,
        };
        let resp = InvokeResponse { result: None, call_log: log, error: Some(format!("unknown tool: {}", req.tool_id)) };
        return (StatusCode::NOT_FOUND, Json(resp));
    }

    // 3. Dispatch
    let t0 = Instant::now();
    let dispatch_result = tools::dispatch(&req.tool_id, &req.args).await;
    let latency_ms = t0.elapsed().as_millis() as u64;

    let (result, result_hash, error) = match dispatch_result {
        Ok(v) => {
            let h = hex::encode(Sha256::digest(serde_json::to_string(&v).unwrap_or_default()));
            (Some(v), h, None)
        }
        Err(e) => (None, String::new(), Some(e.to_string())),
    };

    let log = ToolCallLog {
        call_id: Uuid::new_v4(),
        agent_id: req.agent_id.clone(),
        action_id: req.action_id,
        tool_id: req.tool_id.clone(),
        args_hash,
        result_hash,
        latency_ms,
        policy_decision,
        timestamp,
    };

    tracing::info!(
        agent_id = %req.agent_id,
        tool_id = %req.tool_id,
        latency_ms,
        "tool call completed"
    );

    let status = if error.is_some() { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::OK };
    (status, Json(InvokeResponse { result, call_log: log, error }))
}

async fn handle_list_tools(State(state): State<GatewayState>) -> Json<Value> {
    let tools: Vec<_> = state.registry.list().iter().map(|t| serde_json::json!({
        "id": t.id,
        "description": t.description,
        "class": t.class,
    })).collect();
    Json(serde_json::json!({ "tools": tools }))
}
