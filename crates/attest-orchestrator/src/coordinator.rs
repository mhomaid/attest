//! Coordinator — classifier-path router. No LLM, no destructive tools.
//!
//! Picks which specialists should run for a case given the current verdict
//! and scores. This is the "lightweight routing logic" from `09_Agent_Harness`.

use attest_attestation::Verdict;
use attest_policy_engine::AgentRole;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct CoordinateRequest {
    pub case_id: Option<uuid::Uuid>,
    pub tenant_id: Option<String>,
    #[schema(value_type = String)]
    pub verdict: Verdict,
    pub calibrated_confidence: f32,
    pub novelty_score: f32,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CoordinateDecision {
    pub case_id: uuid::Uuid,
    pub tenant_id: String,
    #[schema(value_type = Vec<String>)]
    pub invoke: Vec<AgentRole>,
    pub reason: String,
    pub execution_path: &'static str,
}

/// Deterministic specialist routing.
pub fn route(req: CoordinateRequest) -> CoordinateDecision {
    let case_id = req.case_id.unwrap_or_else(uuid::Uuid::new_v4);
    let tenant_id = req.tenant_id.unwrap_or_else(|| "default".into());
    let mut invoke = Vec::new();
    let mut reasons: Vec<String> = Vec::new();

    match req.verdict {
        Verdict::NeedsInvestigation | Verdict::EscalatedStub => {
            invoke.push(AgentRole::Investigator);
            reasons.push("verdict needs a deeper investigation".into());
            if req.novelty_score >= 0.70 {
                invoke.push(AgentRole::Hunter);
                reasons.push("novelty is high — hunt related activity".into());
            }
        }
        Verdict::TruePositive => {
            if req.calibrated_confidence >= 0.85 {
                invoke.push(AgentRole::Responder);
                reasons.push("high-confidence true positive — propose containment".into());
            } else {
                invoke.push(AgentRole::Investigator);
                reasons.push("true positive but confidence below 0.85 — investigate first".into());
            }
            if req.novelty_score >= 0.70 {
                invoke.push(AgentRole::Hunter);
                reasons.push("novel true positive — hunt for related activity".into());
            }
        }
        Verdict::FalsePositive | Verdict::Benign => {
            reasons.push("no specialist invocation for a closed benign/FP verdict".into());
        }
    }

    invoke.dedup();
    CoordinateDecision {
        case_id,
        tenant_id,
        invoke,
        reason: reasons.join("; "),
        execution_path: "classifier",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(verdict: Verdict, conf: f32, novelty: f32) -> CoordinateRequest {
        CoordinateRequest {
            case_id: None,
            tenant_id: Some("demo".into()),
            verdict,
            calibrated_confidence: conf,
            novelty_score: novelty,
        }
    }

    #[test]
    fn needs_investigation_routes_to_investigator() {
        let d = route(req(Verdict::NeedsInvestigation, 0.4, 0.2));
        assert_eq!(d.invoke, vec![AgentRole::Investigator]);
    }

    #[test]
    fn novel_needs_investigation_also_hunts() {
        let d = route(req(Verdict::NeedsInvestigation, 0.4, 0.9));
        assert!(d.invoke.contains(&AgentRole::Investigator));
        assert!(d.invoke.contains(&AgentRole::Hunter));
    }

    #[test]
    fn confident_true_positive_routes_to_responder() {
        let d = route(req(Verdict::TruePositive, 0.92, 0.1));
        assert_eq!(d.invoke, vec![AgentRole::Responder]);
    }

    #[test]
    fn benign_invokes_nobody() {
        let d = route(req(Verdict::Benign, 0.99, 0.0));
        assert!(d.invoke.is_empty());
    }
}
