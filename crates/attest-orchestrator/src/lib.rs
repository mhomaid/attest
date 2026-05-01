//! Agent Runtime Orchestrator — implements the Hybrid execution loop.
//!
//! Phase 4b: Full LLM escalation wired via `attest-inference-router` and
//! the MCP gateway. `EscalatedStub` is kept only as a graceful fallback.
//!
//! Entry point: `POST /triage` → `run_triage(request)` → `TriageVerdict`

pub mod agent;
pub mod auto_close;
pub mod calibration;
pub mod guardrails;
pub mod llm_loop;
pub mod mcp_client;
pub mod server;
pub mod shadow_check;
pub mod trace_kafka;
pub mod triage;

pub use agent::{AgentDefinition, AgentRole, ClassifierArtifact, ExecutionPath};
pub use server::build_router;
pub use triage::{TriageRequest, TriageVerdict};
