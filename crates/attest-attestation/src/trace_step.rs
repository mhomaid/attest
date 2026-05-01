//! Live streaming message for analyst workbench trace (Kafka `agent.trace_steps`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::envelope::ExecutionPathKind;

/// Kafka topic for `TraceStep` JSON lines (Redpanda / Kafka).
pub const TRACE_STEPS_TOPIC: &str = "agent.trace_steps";

/// One row streamed over WebSocket to `/ws/cases/{id}/trace`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceStep {
    pub case_id: Uuid,
    pub tenant_id: String,
    pub agent_action_id: Uuid,
    pub agent_id: String,
    pub execution_path: String,
    /// e.g. `tool_call`, `intermediate_belief`, `final_verdict`, `envelope`, `auto_close`, `override`
    pub step_kind: String,
    /// Short summary; producers should cap length (typically ≤200 chars).
    pub summary: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub ts: DateTime<Utc>,
}

impl TraceStep {
    pub fn new(
        case_id: Uuid,
        tenant_id: impl Into<String>,
        agent_action_id: Uuid,
        agent_id: impl Into<String>,
        execution_path: &ExecutionPathKind,
        step_kind: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            case_id,
            tenant_id: tenant_id.into(),
            agent_action_id,
            agent_id: agent_id.into(),
            execution_path: execution_path_snake(execution_path),
            step_kind: step_kind.into(),
            summary: truncate_summary(summary.into(), 200),
            ts: Utc::now(),
        }
    }
}

pub fn execution_path_snake(p: &ExecutionPathKind) -> String {
    match p {
        ExecutionPathKind::Classifier => "classifier",
        ExecutionPathKind::Llm => "llm",
        ExecutionPathKind::Hybrid => "hybrid",
        ExecutionPathKind::HumanOverride => "human_override",
    }
    .to_string()
}

fn truncate_summary(s: String, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s;
    }
    s.chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>()
        + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_step_serializes() {
        let t = TraceStep::new(
            Uuid::from_u128(1),
            "default",
            Uuid::from_u128(2),
            "triager-hybrid-v1",
            &ExecutionPathKind::Hybrid,
            "tool_call",
            "query_hot_tier",
        );
        let j = serde_json::to_string(&t).unwrap();
        assert!(j.contains("tool_call"));
    }
}
