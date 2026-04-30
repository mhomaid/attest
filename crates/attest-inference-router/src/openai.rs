//! OpenAI-compatible chat client.
//!
//! Works with Unsloth Studio (127.0.0.1:8888/v1), llama.cpp server, vLLM, OpenAI, etc.

use crate::{ChatClient, ChatMessage, ChatRequest, ChatResponse, FinishReason, ToolCall, ToolDef, Usage};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct WireRequest<'a> {
    model: &'a str,
    messages: Vec<WireMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<WireTool>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'a str>,
    /// Force non-streaming — Unsloth Studio defaults to SSE streaming.
    stream: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum WireContent {
    Text(String),
    Parts(Vec<Value>),
}

#[derive(Serialize, Deserialize, Debug)]
struct WireMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<WireContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<WireToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct WireToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    function: WireFunction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct WireFunction {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct WireTool {
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireToolFunction,
}

#[derive(Serialize)]
struct WireToolFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Deserialize, Debug)]
struct WireResponse {
    choices: Vec<WireChoice>,
    #[serde(default)]
    usage: Option<WireUsage>,
}

#[derive(Deserialize, Debug)]
struct WireChoice {
    message: WireMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct WireUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// ── Conversions ───────────────────────────────────────────────────────────────

fn to_wire_messages(msgs: &[ChatMessage]) -> Vec<WireMessage> {
    msgs.iter().map(|m| match m {
        ChatMessage::System { content } => WireMessage {
            role: "system".into(),
            content: Some(WireContent::Text(content.clone())),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        ChatMessage::User { content } => WireMessage {
            role: "user".into(),
            content: Some(WireContent::Text(content.clone())),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        ChatMessage::Assistant { content, tool_calls } => WireMessage {
            role: "assistant".into(),
            content: Some(WireContent::Text(content.clone())),
            tool_calls: tool_calls.as_ref().map(|tc| {
                tc.iter().map(|c| WireToolCall {
                    id: c.id.clone(),
                    kind: "function".into(),
                    function: WireFunction {
                        name: c.name.clone(),
                        arguments: serde_json::to_string(&c.arguments).unwrap_or_default(),
                    },
                }).collect()
            }),
            tool_call_id: None,
            name: None,
        },
        ChatMessage::Tool { tool_call_id, content } => WireMessage {
            role: "tool".into(),
            content: Some(WireContent::Text(content.clone())),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.clone()),
            name: None,
        },
    }).collect()
}

fn to_wire_tools(tools: &[ToolDef]) -> Vec<WireTool> {
    tools.iter().map(|t| WireTool {
        kind: "function",
        function: WireToolFunction {
            name: t.name.clone(),
            description: t.description.clone(),
            parameters: t.parameters.clone(),
        },
    }).collect()
}

fn parse_finish_reason(s: Option<&str>) -> FinishReason {
    match s {
        Some("stop") => FinishReason::Stop,
        Some("tool_calls") => FinishReason::ToolCalls,
        Some("length") => FinishReason::Length,
        Some(other) => FinishReason::Other(other.into()),
        None => FinishReason::Stop,
    }
}

// ── Client ────────────────────────────────────────────────────────────────────

pub struct OpenAiCompatClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl OpenAiCompatClient {
    /// Create a new client.
    ///
    /// `base_url` should point to the root of the OpenAI-compat API, e.g.
    /// `http://127.0.0.1:8888/v1`.
    pub fn new(base_url: impl Into<String>, model: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("failed to build reqwest client"),
            api_key,
        }
    }
}

#[async_trait]
impl ChatClient for OpenAiCompatClient {
    fn provider(&self) -> &str { "local" }
    fn model_id(&self) -> &str { &self.model }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        let t0 = Instant::now();

        let has_tools = !req.tools.is_empty();
        let wire_req = WireRequest {
            model: &self.model,
            messages: to_wire_messages(&req.messages),
            tools: to_wire_tools(&req.tools),
            max_tokens: req.max_tokens,
            temperature: req.temperature,
            tool_choice: if has_tools { Some("auto") } else { None },
            stream: false,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let mut rb = self.client.post(&url).json(&wire_req);
        if let Some(key) = &self.api_key {
            rb = rb.bearer_auth(key);
        }

        let http_resp = rb.send().await.context("OpenAI-compat: HTTP request failed")?;
        let status = http_resp.status();
        let body: Value = http_resp.json().await.context("OpenAI-compat: failed to parse response")?;

        if !status.is_success() {
            anyhow::bail!("OpenAI-compat API error {status}: {body}");
        }

        let wire: WireResponse = serde_json::from_value(body.clone())
            .with_context(|| format!("OpenAI-compat: unexpected response shape: {body}"))?;

        let choice = wire.choices.into_iter().next()
            .context("OpenAI-compat: empty choices")?;
        let msg = choice.message;

        let content = match &msg.content {
            Some(WireContent::Text(t)) => t.clone(),
            _ => String::new(),
        };

        let tool_calls = msg.tool_calls.unwrap_or_default().into_iter().map(|tc| {
            let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
            ToolCall { id: tc.id, name: tc.function.name, arguments: args }
        }).collect::<Vec<_>>();

        let finish_reason = parse_finish_reason(choice.finish_reason.as_deref());

        let usage = wire.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }).unwrap_or_default();

        Ok(ChatResponse {
            content,
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
    use crate::{ToolDef};
    use serde_json::json;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::{method, path}};

    fn tool_def() -> ToolDef {
        ToolDef {
            name: "get_user_baseline".into(),
            description: "Get user baseline".into(),
            parameters: json!({ "type": "object", "properties": { "principal": { "type": "string" } }, "required": ["principal"] }),
        }
    }

    #[tokio::test]
    async fn openai_compat_plain_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": { "role": "assistant", "content": "verdict: benign" },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
            })))
            .mount(&server)
            .await;

        let client = OpenAiCompatClient::new(server.uri(), "test-model", None);
        let req = ChatRequest::new(
            vec![ChatMessage::user("classify this alert")],
            vec![],
        );
        let resp = client.chat(req).await.unwrap();
        assert_eq!(resp.content, "verdict: benign");
        assert_eq!(resp.finish_reason, FinishReason::Stop);
        assert!(resp.tool_calls.is_empty());
        assert_eq!(resp.usage.total_tokens, 15);
    }

    #[tokio::test]
    async fn openai_compat_tool_call_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_abc",
                            "type": "function",
                            "function": {
                                "name": "get_user_baseline",
                                "arguments": "{\"principal\": \"alice@example.com\"}"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }],
                "usage": { "prompt_tokens": 20, "completion_tokens": 15, "total_tokens": 35 }
            })))
            .mount(&server)
            .await;

        let client = OpenAiCompatClient::new(server.uri(), "test-model", None);
        let req = ChatRequest::new(
            vec![ChatMessage::user("classify this alert")],
            vec![tool_def()],
        );
        let resp = client.chat(req).await.unwrap();
        assert!(resp.content.is_empty());
        assert_eq!(resp.finish_reason, FinishReason::ToolCalls);
        assert_eq!(resp.tool_calls.len(), 1);
        let tc = &resp.tool_calls[0];
        assert_eq!(tc.id, "call_abc");
        assert_eq!(tc.name, "get_user_baseline");
        assert_eq!(tc.arguments["principal"], "alice@example.com");
    }

    #[tokio::test]
    async fn openai_compat_api_error_surfaces() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(429).set_body_json(json!({
                "error": { "message": "rate limited" }
            })))
            .mount(&server)
            .await;

        let client = OpenAiCompatClient::new(server.uri(), "test-model", None);
        let req = ChatRequest::new(vec![ChatMessage::user("test")], vec![]);
        let err = client.chat(req).await.unwrap_err();
        assert!(err.to_string().contains("429"));
    }
}
