//! MCP gateway client.
//!
//! Thin `reqwest` wrapper over `POST /invoke` on `attest-mcp-gateway`.
//! Every call is policy-checked and logged server-side; the client only
//! needs to forward the agent context and arguments.

use anyhow::{Context, Result};
use attest_attestation::ToolCallRecord;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::Instant;
use uuid::Uuid;

/// A successful or policy-denied tool invocation.
#[derive(Debug)]
pub struct McpInvokeResult {
    /// JSON result from the tool (None if denied or errored).
    pub result: Option<Value>,
    /// Error message (None on success).
    pub error: Option<String>,
    /// Structured log entry for inclusion in `LlmEvidence::tool_calls`.
    pub record: ToolCallRecord,
}

#[derive(Serialize)]
struct InvokeRequest<'a> {
    agent_id: &'a str,
    action_id: Uuid,
    agent_role: &'a str,
    tool_id: &'a str,
    args: &'a Value,
    calibrated_confidence: f32,
}

#[derive(Deserialize)]
struct InvokeResponse {
    result: Option<Value>,
    call_log: CallLog,
    error: Option<String>,
}

#[derive(Deserialize)]
struct CallLog {
    #[serde(rename = "call_id")]
    _call_id: Uuid,
    #[serde(rename = "args_hash")]
    _args_hash: String,
    result_hash: String,
    latency_ms: u64,
    policy_decision: Value,
    timestamp: chrono::DateTime<Utc>,
}

pub struct McpClient {
    base_url: String,
    client: reqwest::Client,
}

impl McpClient {
    /// Create a new client. `base_url` e.g. `http://localhost:4500`.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build MCP reqwest client"),
        }
    }

    pub fn from_env() -> Self {
        let url = std::env::var("MCP_GATEWAY_URL")
            .unwrap_or_else(|_| "http://localhost:4242".into());
        Self::new(url)
    }

    /// Invoke a tool through the MCP gateway.
    pub async fn invoke(
        &self,
        agent_id: &str,
        action_id: Uuid,
        agent_role: &str,
        tool_id: &str,
        args: &Value,
        calibrated_confidence: f32,
    ) -> Result<McpInvokeResult> {
        let t0 = Instant::now();

        let body = InvokeRequest {
            agent_id,
            action_id,
            agent_role,
            tool_id,
            args,
            calibrated_confidence,
        };

        let mut req = self
            .client
            .post(format!("{}/invoke", self.base_url))
            .json(&body);

        if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
        {
            let mut headers = http::HeaderMap::new();
            attest_telemetry::inject_trace_headers(&mut headers);
            req = req.headers(headers);
        }

        let http_resp = req
            .send()
            .await
            .with_context(|| format!("MCP gateway unreachable at {}", self.base_url))?;

        let latency_ms = t0.elapsed().as_millis() as u64;
        let status = http_resp.status();
        let resp: InvokeResponse = http_resp.json().await
            .context("failed to parse MCP gateway response")?;

        let policy_str = match &resp.call_log.policy_decision {
            v if v.get("kind").and_then(|k| k.as_str()) == Some("allow") => "allow".to_string(),
            v => v.to_string(),
        };

        let args_hash = hex::encode(Sha256::digest(
            serde_json::to_string(args).unwrap_or_default()
        ));
        let result_hash = resp.call_log.result_hash.clone();

        tracing::debug!(
            tool_id,
            latency_ms,
            policy = %policy_str,
            success = resp.error.is_none(),
            "MCP tool call"
        );

        if !status.is_success() && resp.error.is_none() {
            return Err(anyhow::anyhow!(
                "MCP gateway returned {status} for tool {tool_id}"
            ));
        }

        let record = ToolCallRecord {
            tool_id: tool_id.to_string(),
            args_hash,
            result_hash,
            latency_ms: resp.call_log.latency_ms,
            policy_decision: policy_str,
            timestamp: resp.call_log.timestamp,
        };

        Ok(McpInvokeResult {
            result: resp.result,
            error: resp.error,
            record,
        })
    }
}
