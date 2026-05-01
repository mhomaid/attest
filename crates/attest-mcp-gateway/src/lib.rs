//! MCP Gateway — single entry point for every agent tool call.
//!
//! Responsibilities:
//! - Authenticate every tool call against the policy engine
//! - Log every invocation (agent_id, tool_id, args hash, result hash, latency, policy decision)
//! - Dispatch to internal Attest tools
//! - Rate-limit per agent/tenant

pub mod registry;
pub mod server;
pub mod tools;
pub mod warm_limit;

pub use registry::{ToolDescriptor, ToolRegistry};
pub use server::build_router;
pub use warm_limit::WarmTierLimiter;
