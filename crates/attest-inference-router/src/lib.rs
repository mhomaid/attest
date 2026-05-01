//! Unified LLM inference client for the Attest orchestrator.
//!
//! Abstracts over:
//!  - OpenAI-compatible endpoints (Unsloth Studio, llama.cpp, vLLM, etc.)
//!  - Anthropic Messages API
//!
//! Selected at startup via `ATTEST_LLM_PROVIDER` (`local` | `anthropic`).
//!
//! # Usage
//!
//! ```rust,no_run
//! use attest_inference_router::from_env;
//!
//! #[tokio::main]
//! async fn main() {
//!     let client = from_env().unwrap();
//!     println!("Using provider: {}", client.provider());
//! }
//! ```

pub mod anthropic;
pub mod openai;

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

// ── Common types ─────────────────────────────────────────────────────────────

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum ChatMessage {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: String,
        tool_calls: Option<Vec<ToolCall>>,
    },
    /// Tool result message returned after executing a tool call.
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self::System {
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self::User {
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::Assistant {
            content: content.into(),
            tool_calls: None,
        }
    }
    pub fn tool_result(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Tool {
            tool_call_id: id.into(),
            content: content.into(),
        }
    }
}

/// A tool call requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// A tool definition exposed to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool's input arguments.
    pub parameters: Value,
}

/// Request to the chat API.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDef>,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Temperature (0.0–2.0).
    pub temperature: f32,
}

impl ChatRequest {
    pub fn new(messages: Vec<ChatMessage>, tools: Vec<ToolDef>) -> Self {
        Self {
            messages,
            tools,
            max_tokens: 2048,
            temperature: 0.1,
        }
    }
}

/// Token usage reported by the API.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Finish reason returned by the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    /// Model produced a final text response.
    Stop,
    /// Model requested one or more tool calls.
    ToolCalls,
    /// Response was truncated by max_tokens.
    Length,
    /// Unknown or provider-specific reason.
    Other(String),
}

/// Response from the chat API.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    /// Final text content (empty if tool_calls is non-empty).
    pub content: String,
    /// Tool calls requested by the model (empty if content is non-empty).
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: FinishReason,
    pub usage: Usage,
    /// Wall-clock latency of this specific API call.
    pub latency: Duration,
}

impl ChatResponse {
    /// True when the model wants to call tools rather than produce a final answer.
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

// ── ChatClient trait ──────────────────────────────────────────────────────────

/// Unified asynchronous chat client.
#[async_trait]
pub trait ChatClient: Send + Sync {
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse>;
    fn provider(&self) -> &str;
    fn model_id(&self) -> &str;
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Build a `ChatClient` from environment variables:
///
/// | Variable | Default | Notes |
/// |---|---|---|
/// | `ATTEST_LLM_PROVIDER` | `local` | `local` or `anthropic` |
/// | `ATTEST_LLM_BASE_URL` | `http://127.0.0.1:8888/v1` | OpenAI-compat base URL (for `local`) |
/// | `ATTEST_LLM_MODEL` | `unsloth/Qwen3.6-35B-A3B-GGUF` | Model ID |
/// | `ATTEST_OPENAI_NATIVE_TOOL_MESSAGES` | unset (`false`) | Set to `1`/`true` to send OpenAI-native `role: "tool"` messages (Unsloth often rejects these; default wraps results as `user`) |
/// | `ANTHROPIC_API_KEY` | — | Required when provider is `anthropic` |
pub fn from_env() -> Result<Box<dyn ChatClient>> {
    let provider = std::env::var("ATTEST_LLM_PROVIDER").unwrap_or_else(|_| "local".into());

    match provider.as_str() {
        "local" => {
            let base_url = std::env::var("ATTEST_LLM_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8888/v1".into());
            let model = std::env::var("ATTEST_LLM_MODEL")
                .unwrap_or_else(|_| "unsloth/Qwen3.6-35B-A3B-GGUF".into());
            // Optional bearer token for Unsloth Studio (Settings → API Keys)
            let api_key = std::env::var("ATTEST_LLM_API_KEY").ok();
            let native_tool_messages = std::env::var("ATTEST_OPENAI_NATIVE_TOOL_MESSAGES")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let openai_config = openai::OpenAiCompatConfig {
                encode_tool_results_as_user: !native_tool_messages,
            };
            tracing::info!(
                base_url,
                model,
                native_tool_messages,
                "LLM provider: local (OpenAI-compat)"
            );
            Ok(Box::new(openai::OpenAiCompatClient::with_config(
                base_url,
                model,
                api_key,
                openai_config,
            )))
        }
        "anthropic" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                anyhow::anyhow!("ANTHROPIC_API_KEY must be set when ATTEST_LLM_PROVIDER=anthropic")
            })?;
            let model =
                std::env::var("ATTEST_LLM_MODEL").unwrap_or_else(|_| "claude-sonnet-4-5".into());
            tracing::info!(model, "LLM provider: anthropic");
            Ok(Box::new(anthropic::AnthropicClient::new(api_key, model)))
        }
        other => bail!("unknown ATTEST_LLM_PROVIDER={other:?}; valid values: local, anthropic"),
    }
}
