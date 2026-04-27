//! Agent Runtime Orchestrator — implements the Hybrid execution loop.
//!
//! Phase 4a: Classifier path is fully wired. LLM escalation is a stub
//! (EscalatedStub envelope variant); real Claude wiring comes in Phase 4b.
//!
//! Entry point: `POST /triage` → `run_triage(request)` → `TriageVerdict`

pub mod agent;
pub mod calibration;
pub mod server;
pub mod triage;

pub use agent::{AgentDefinition, AgentRole, ClassifierArtifact, ExecutionPath};
pub use server::build_router;
pub use triage::{TriageRequest, TriageVerdict};
