//! Policy engine — deterministic authorization for every agent tool call.
//!
//! Hard-coded Rust policies per `AgentRole` for MVP. GA v1 will expose
//! customer-editable policies via a typed DSL.

use serde::{Deserialize, Serialize};

/// Every agent role in the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Triager,
    Investigator,
    Hunter,
    DetectionEngineer,
    Responder,
    Coordinator,
}

/// The authorization decision returned by the policy engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PolicyDecision {
    Allow,
    Deny {
        reason: String,
    },
    /// Action requires human approval before proceeding.
    Escalate {
        reason: String,
    },
}

impl PolicyDecision {
    pub fn allow() -> Self {
        Self::Allow
    }
    pub fn deny(reason: impl Into<String>) -> Self {
        Self::Deny {
            reason: reason.into(),
        }
    }
    pub fn escalate(reason: impl Into<String>) -> Self {
        Self::Escalate {
            reason: reason.into(),
        }
    }
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

/// Contextual inputs evaluated by the policy.
#[derive(Debug, Clone, Default)]
pub struct PolicyContext {
    /// Calibrated probability (0–1) that the verdict is correct.
    pub calibrated_confidence: f32,
    /// Whether the target principal is on the do-not-touch list.
    pub target_is_protected: bool,
    /// Number of automated actions taken in the last hour.
    pub recent_actions_last_hour: u32,
    /// Estimated number of principals / assets affected by this action.
    pub blast_radius: u32,
    /// Whether the issuing tenant has enabled automated actions.
    pub tenant_allows_automation: bool,
}

/// Authorise a tool call for the given agent role.
///
/// Returns `PolicyDecision::Allow`, `Deny`, or `Escalate`.
pub fn authorize(role: &AgentRole, tool_id: &str, ctx: &PolicyContext) -> PolicyDecision {
    // All roles: read-only hot/warm tier and baseline queries are always allowed.
    if matches!(
        tool_id,
        "query_hot_tier"
            | "query_warm_tier"
            | "get_user_baseline"
            | "get_asset_context"
            | "lookup_threat_intel"
            | "analyze_code_snippet"
            | "sandbox_detonate"
    ) {
        return PolicyDecision::allow();
    }

    match role {
        AgentRole::Triager => authorize_triager(tool_id, ctx),
        AgentRole::Investigator => authorize_investigator(tool_id, ctx),
        AgentRole::DetectionEngineer => authorize_detection_engineer(tool_id, ctx),
        AgentRole::Responder => authorize_responder(tool_id, ctx),
        AgentRole::Hunter => authorize_hunter(tool_id, ctx),
        AgentRole::Coordinator => {
            // Coordinator is lightweight routing logic — no destructive tools.
            PolicyDecision::deny(format!("Coordinator role may not invoke {tool_id}"))
        }
    }
}

fn authorize_triager(tool_id: &str, ctx: &PolicyContext) -> PolicyDecision {
    match tool_id {
        "triager_auto_close" => {
            if !ctx.tenant_allows_automation {
                return PolicyDecision::deny("tenant has disabled automated actions");
            }
            if ctx.calibrated_confidence < 0.90 {
                return PolicyDecision::deny(format!(
                    "auto-close requires confidence ≥ 0.90; got {:.2}",
                    ctx.calibrated_confidence
                ));
            }
            PolicyDecision::allow()
        }
        _ => PolicyDecision::deny(format!("Triager may not invoke {tool_id}")),
    }
}

fn authorize_investigator(tool_id: &str, _ctx: &PolicyContext) -> PolicyDecision {
    match tool_id {
        "propose_escalation" | "request_human_review" => PolicyDecision::allow(),
        _ => PolicyDecision::deny(format!("Investigator may not invoke {tool_id}")),
    }
}

fn authorize_detection_engineer(tool_id: &str, ctx: &PolicyContext) -> PolicyDecision {
    match tool_id {
        "propose_detection_pr" => PolicyDecision::allow(),
        "deploy_detection" => {
            if ctx.calibrated_confidence < 0.80 {
                return PolicyDecision::deny(format!(
                    "detection deploy requires confidence ≥ 0.80; got {:.2}",
                    ctx.calibrated_confidence
                ));
            }
            PolicyDecision::allow()
        }
        _ => PolicyDecision::deny(format!("DetectionEngineer may not invoke {tool_id}")),
    }
}

fn authorize_responder(tool_id: &str, ctx: &PolicyContext) -> PolicyDecision {
    match tool_id {
        "idp_revoke_session" => {
            if ctx.calibrated_confidence < 0.85 {
                return PolicyDecision::deny(format!(
                    "idp_revoke_session requires confidence ≥ 0.85; got {:.2}",
                    ctx.calibrated_confidence
                ));
            }
            if ctx.target_is_protected {
                return PolicyDecision::deny("target is on the do-not-touch list");
            }
            if ctx.recent_actions_last_hour > 10 {
                return PolicyDecision::deny("rate limit: max 10 automated actions/hour");
            }
            if ctx.blast_radius > 50 {
                return PolicyDecision::escalate(format!(
                    "blast radius {} exceeds 50; human approval required",
                    ctx.blast_radius
                ));
            }
            PolicyDecision::allow()
        }
        "edr_isolate_host" => {
            if ctx.target_is_protected {
                return PolicyDecision::deny("host is on the production-critical list");
            }
            if ctx.blast_radius > 1 {
                return PolicyDecision::escalate(
                    "edr isolation of >1 host requires human approval",
                );
            }
            PolicyDecision::allow()
        }
        "firewall_block_ioc" => PolicyDecision::allow(),
        _ => PolicyDecision::deny(format!("Responder may not invoke {tool_id}")),
    }
}

fn authorize_hunter(tool_id: &str, _ctx: &PolicyContext) -> PolicyDecision {
    match tool_id {
        "propose_detection_pr" | "request_human_review" => PolicyDecision::allow(),
        _ => PolicyDecision::deny(format!("Hunter may not invoke {tool_id}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> PolicyContext {
        PolicyContext {
            calibrated_confidence: 0.95,
            target_is_protected: false,
            recent_actions_last_hour: 0,
            blast_radius: 1,
            tenant_allows_automation: true,
        }
    }

    #[test]
    fn read_tools_always_allowed() {
        for role in [
            AgentRole::Triager,
            AgentRole::Responder,
            AgentRole::Coordinator,
        ] {
            assert!(authorize(&role, "query_hot_tier", &ctx()).is_allowed());
            assert!(authorize(&role, "get_user_baseline", &ctx()).is_allowed());
        }
    }

    #[test]
    fn responder_idp_revoke_blocked_by_confidence() {
        let mut c = ctx();
        c.calibrated_confidence = 0.70;
        assert!(!authorize(&AgentRole::Responder, "idp_revoke_session", &c).is_allowed());
    }

    #[test]
    fn responder_idp_revoke_escalates_on_blast_radius() {
        let mut c = ctx();
        c.blast_radius = 100;
        assert!(matches!(
            authorize(&AgentRole::Responder, "idp_revoke_session", &c),
            PolicyDecision::Escalate { .. }
        ));
    }

    #[test]
    fn triager_auto_close_allowed_when_confident() {
        assert!(authorize(&AgentRole::Triager, "triager_auto_close", &ctx()).is_allowed());
    }

    #[test]
    fn coordinator_may_not_invoke_destructive_tools() {
        assert!(!authorize(&AgentRole::Coordinator, "idp_revoke_session", &ctx()).is_allowed());
    }
}
