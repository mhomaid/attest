//! Anthropic Messages API client.
//!
//! Handles Anthropic's `content` block format (where tool use is a typed block,
//! not a top-level `tool_calls` array) and normalises into the shared `ChatResponse`.

use crate::{ChatClient, ChatMessage, ChatRequest, ChatResponse, FinishReason, ToolCall, ToolDef, Usage};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value};
use std::time::Instant;

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Serialize, Debug)]
struct WireRequest<'a> {
    model: &'a str,
    messages: Vec<WireMessage>,
    system: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<WireTool>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize, Deserialize, Debug)]
struct WireMessage {
    role: String,
    content: WireContent,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum WireContent {
    Text(String),
    Blocks(Vec<WireContentBlock>),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Serialize, Debug)]
struct WireTool {
    name: String,
    description: String,
    input_schema: Value,
}

#[derive(Deserialize, Debug)]
struct WireResponse {
    content: Vec<WireContentBlock>,
    stop_reason: Option<String>,
    #[serde(default)]
    usage: Option<WireUsage>,
}

#[derive(Deserialize, Debug)]
struct WireUsage {
    input_tokens: u32,
    output_tokens: u32,
}

// ── Conversions ───────────────────────────────────────────────────────────────

/// Build Anthropic wire messages from the shared `ChatMessage` list.
/// Anthropic requires the system prompt to be passed separately; it is extracted
/// and returned alongside the message list.
fn to_wire(msgs: &[ChatMessage]) -> (Option<String>, Vec<WireMessage>) {
    let mut system = None;
    let mut out = Vec::new();

    for m in msgs {
        match m {
            ChatMessage::System { content } => {
                system = Some(content.clone());
            }
            ChatMessage::User { content } => {
                out.push(WireMessage {
                    role: "user".into(),
                    content: WireContent::Text(content.clone()),
                });
            }
            ChatMessage::Assistant { content, tool_calls } => {
                let mut blocks: Vec<WireContentBlock> = Vec::new();
                if !content.is_empty() {
                    blocks.push(WireContentBlock::Text { text: content.clone() });
                }
                if let Some(tcs) = tool_calls {
                    for tc in tcs {
                        blocks.push(WireContentBlock::ToolUse {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                            input: tc.arguments.clone(),
                        });
                    }
                }
                out.push(WireMessage {
                    role: "assistant".into(),
                    content: if blocks.len() == 1 {
                        // If it's only text, use the string form for cleaner payloads
                        if let WireContentBlock::Text { text } = &blocks[0] {
                            WireContent::Text(text.clone())
                        } else {
                            WireContent::Blocks(blocks)
                        }
                    } else {
                        WireContent::Blocks(blocks)
                    },
                });
            }
            ChatMessage::Tool { tool_call_id, content } => {
                // Anthropic tool results go as a user message with a tool_result block
                out.push(WireMessage {
                    role: "user".into(),
                    content: WireContent::Blocks(vec![WireContentBlock::ToolResult {
                        tool_use_id: tool_call_id.clone(),
                        content: content.clone(),
                    }]),
                });
            }
        }
    }

    (system, out)
}

fn to_wire_tools(tools: &[ToolDef]) -> Vec<WireTool> {
    tools.iter().map(|t| WireTool {
        name: t.name.clone(),
        description: t.description.clone(),
        input_schema: t.parameters.clone(),
    }).collect()
}

fn parse_finish_reason(s: Option<&str>) -> FinishReason {
    match s {
        Some("end_turn") => FinishReason::Stop,
        Some("tool_use") => FinishReason::ToolCalls,
        Some("max_tokens") => FinishReason::Length,
        Some(other) => FinishReason::Other(other.into()),
        None => FinishReason::Stop,
    }
}

// ── Client ────────────────────────────────────────────────────────────────────

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    api_key: String,
    model: String,
    client: reqwest::Client,
    /// Override endpoint (used for testing).
    endpoint: String,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("failed to build reqwest client"),
            endpoint: ANTHROPIC_API_URL.into(),
        }
    }

    /// Used in tests to point at a mock server.
    #[cfg(test)]
    pub fn with_endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint = url.into();
        self
    }
}

#[async_trait]
impl ChatClient for AnthropicClient {
    fn provider(&self) -> &str { "anthropic" }
    fn model_id(&self) -> &str { &self.model }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        let t0 = Instant::now();

        let (system, messages) = to_wire(&req.messages);
        let wire_req = WireRequest {
            model: &self.model,
            messages,
            system,
            tools: to_wire_tools(&req.tools),
            max_tokens: req.max_tokens,
            temperature: req.temperature,
        };

        let http_resp = self.client
            .post(&self.endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&wire_req)
            .send()
            .await
            .context("Anthropic: HTTP request failed")?;

        let status = http_resp.status();
        let body: Value = http_resp.json().await.context("Anthropic: failed to parse response")?;

        if !status.is_success() {
            anyhow::bail!("Anthropic API error {status}: {body}");
        }

        let wire: WireResponse = serde_json::from_value(body.clone())
            .with_context(|| format!("Anthropic: unexpected response shape: {body}"))?;

        let mut content_text = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        for block in wire.content {
            match block {
                WireContentBlock::Text { text } => {
                    if !content_text.is_empty() { content_text.push('\n'); }
                    content_text.push_str(&text);
                }
                WireContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall { id, name, arguments: input });
                }
                WireContentBlock::ToolResult { .. } => {}
            }
        }

        let finish_reason = parse_finish_reason(wire.stop_reason.as_deref());
        let usage = wire.usage.map(|u| Usage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.input_tokens + u.output_tokens,
        }).unwrap_or_default();

        Ok(ChatResponse {
            content: content_text,
            tool_calls,
            finish_reason,
            usage,
            latency: t0.elapsed(),
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::{method, path}};

    fn tool_def() -> ToolDef {
        ToolDef {
            name: "get_user_baseline".into(),
            description: "Get user baseline".into(),
            parameters: json!({ "type": "object", "properties": { "principal": { "type": "string" } } }),
        }
    }

    #[tokio::test]
    async fn anthropic_plain_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": [{ "type": "text", "text": "{\"verdict\": \"benign\"}" }],
                "stop_reason": "end_turn",
                "usage": { "input_tokens": 100, "output_tokens": 20 }
            })))
            .mount(&server)
            .await;

        let client = AnthropicClient::new("test-key", "claude-sonnet-4-5")
            .with_endpoint(format!("{}/v1/messages", server.uri()));
        let req = ChatRequest::new(
            vec![ChatMessage::user("classify this alert")],
            vec![],
        );
        let resp = client.chat(req).await.unwrap();
        assert!(resp.content.contains("benign"));
        assert_eq!(resp.finish_reason, FinishReason::Stop);
        assert!(resp.tool_calls.is_empty());
        assert_eq!(resp.usage.completion_tokens, 20);
    }

    #[tokio::test]
    async fn anthropic_tool_use_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_01",
                    "name": "get_user_baseline",
                    "input": { "principal": "alice@example.com" }
                }],
                "stop_reason": "tool_use",
                "usage": { "input_tokens": 50, "output_tokens": 30 }
            })))
            .mount(&server)
            .await;

        let client = AnthropicClient::new("test-key", "claude-sonnet-4-5")
            .with_endpoint(format!("{}/v1/messages", server.uri()));
        let req = ChatRequest::new(
            vec![ChatMessage::user("classify this alert")],
            vec![tool_def()],
        );
        let resp = client.chat(req).await.unwrap();
        assert!(resp.content.is_empty());
        assert_eq!(resp.finish_reason, FinishReason::ToolCalls);
        assert_eq!(resp.tool_calls.len(), 1);
        let tc = &resp.tool_calls[0];
        assert_eq!(tc.id, "toolu_01");
        assert_eq!(tc.name, "get_user_baseline");
        assert_eq!(tc.arguments["principal"], "alice@example.com");
    }

    #[tokio::test]
    async fn anthropic_mixed_text_and_tool() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": [
                    { "type": "text", "text": "I need to look this up." },
                    { "type": "tool_use", "id": "toolu_02", "name": "lookup_threat_intel", "input": { "indicator": "1.2.3.4", "indicator_type": "ip" } }
                ],
                "stop_reason": "tool_use",
                "usage": { "input_tokens": 60, "output_tokens": 40 }
            })))
            .mount(&server)
            .await;

        let client = AnthropicClient::new("test-key", "claude-sonnet-4-5")
            .with_endpoint(format!("{}/v1/messages", server.uri()));
        let req = ChatRequest::new(vec![ChatMessage::user("analyze")], vec![tool_def()]);
        let resp = client.chat(req).await.unwrap();
        // Mixed: has both text and tool calls
        assert!(!resp.content.is_empty());
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "lookup_threat_intel");
    }

    #[tokio::test]
    async fn anthropic_api_error_surfaces() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": { "type": "authentication_error", "message": "invalid api key" }
            })))
            .mount(&server)
            .await;

        let client = AnthropicClient::new("bad-key", "claude-sonnet-4-5")
            .with_endpoint(format!("{}/v1/messages", server.uri()));
        let req = ChatRequest::new(vec![ChatMessage::user("test")], vec![]);
        let err = client.chat(req).await.unwrap_err();
        assert!(err.to_string().contains("401"));
    }
}
