//! Shadow check verifier — deterministic gate before any Triager auto-close.
//!
//! The shadow check evaluates a fixed set of policies against the triage context
//! and returns `ShadowCheckDecision { allowed, reason, policies_evaluated }`.
//!
//! Policy evaluation delegates to `attest-policy-engine::authorize` for the
//! `triager_auto_close` tool, then enriches the context with:
//!   - Do-not-touch list (comma-separated `DO_NOT_TOUCH_LIST` env var)
//!   - Action-class allowlist (only `login`, `api_call`, `file_access` auto-close by default)
//!   - Tenant automation flag (`TENANT_ALLOWS_AUTOMATION` env var, default `true`)

use attest_attestation::ShadowCheckDecision;
use attest_policy_engine::{authorize, AgentRole, PolicyContext, PolicyDecision};
use std::collections::HashSet;

/// Runtime shadow-checker loaded once at orchestrator startup.
#[derive(Debug, Clone)]
pub struct ShadowChecker {
    /// Principals that must never be auto-closed (exact match, lower-case).
    do_not_touch: HashSet<String>,
    /// Whether this tenant has enabled automated actions at all.
    tenant_allows_automation: bool,
    /// Action classes that are eligible for auto-close.
    allowed_action_classes: HashSet<String>,
}

impl ShadowChecker {
    /// Build from individual values.
    pub fn new(
        do_not_touch: HashSet<String>,
        _auto_close_threshold: f32,
        tenant_allows_automation: bool,
        allowed_action_classes: HashSet<String>,
    ) -> Self {
        Self { do_not_touch, tenant_allows_automation, allowed_action_classes }
    }

    /// Build from environment variables.
    ///
    /// | Env var | Default |
    /// |---|---|
    /// | `DO_NOT_TOUCH_LIST` | `` (empty) |
    /// | `AUTO_CLOSE_THRESHOLD` | `0.90` |
    /// | `TENANT_ALLOWS_AUTOMATION` | `true` |
    /// | `AUTO_CLOSE_ACTION_CLASSES` | `login,api_call,file_access` |
    pub fn from_env() -> Self {
        let do_not_touch: HashSet<String> = std::env::var("DO_NOT_TOUCH_LIST")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let tenant_allows_automation: bool = std::env::var("TENANT_ALLOWS_AUTOMATION")
            .map(|v| !matches!(v.to_ascii_lowercase().as_str(), "false" | "0" | "off"))
            .unwrap_or(true);

        let allowed_action_classes: HashSet<String> =
            std::env::var("AUTO_CLOSE_ACTION_CLASSES")
                .unwrap_or_else(|_| "login,api_call,file_access".into())
                .split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect();

        Self::new(do_not_touch, 0.90, tenant_allows_automation, allowed_action_classes)
    }

    /// Evaluate all shadow-check policies against the given triage context.
    ///
    /// # Arguments
    /// * `principal` — The acting principal extracted from the alert (e.g. `alice@corp.com`).
    /// * `action_class` — The type of activity (e.g. `login`, `api_call`).
    /// * `calibrated_confidence` — Calibrated probability from the triage pipeline.
    pub fn check(
        &self,
        principal: &str,
        action_class: &str,
        calibrated_confidence: f32,
    ) -> ShadowCheckDecision {
        let mut policies_evaluated: Vec<String> = Vec::new();

        // Policy 1: Do-not-touch list
        policies_evaluated.push("do_not_touch".into());
        let target_is_protected = self.do_not_touch.contains(&principal.to_ascii_lowercase());
        if target_is_protected {
            return ShadowCheckDecision {
                allowed: false,
                reason: format!("principal '{principal}' is on the do-not-touch list"),
                policies_evaluated,
            };
        }

        // Policy 2: Action-class allowlist
        policies_evaluated.push("action_class_allowlist".into());
        if !self.allowed_action_classes.contains(&action_class.to_ascii_lowercase()) {
            return ShadowCheckDecision {
                allowed: false,
                reason: format!(
                    "action class '{action_class}' is not in the auto-close allowlist"
                ),
                policies_evaluated,
            };
        }

        // Policy 3: Tenant automation flag + confidence gate (via policy engine)
        policies_evaluated.push("policy_engine_triager_auto_close".into());
        let ctx = PolicyContext {
            calibrated_confidence,
            target_is_protected: false, // already checked above
            recent_actions_last_hour: 0,
            blast_radius: 1,
            tenant_allows_automation: self.tenant_allows_automation,
        };

        match authorize(&AgentRole::Triager, "triager_auto_close", &ctx) {
            PolicyDecision::Allow => ShadowCheckDecision {
                allowed: true,
                reason: format!(
                    "all policies passed (confidence={calibrated_confidence:.3}, \
                     action_class={action_class})"
                ),
                policies_evaluated,
            },
            PolicyDecision::Deny { reason } => ShadowCheckDecision {
                allowed: false,
                reason,
                policies_evaluated,
            },
            PolicyDecision::Escalate { reason } => ShadowCheckDecision {
                allowed: false,
                reason: format!("policy engine escalated: {reason}"),
                policies_evaluated,
            },
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> ShadowChecker {
        ShadowChecker::new(
            ["ceo@corp.com".into()].into(),
            0.90,
            true,
            ["login".into(), "api_call".into()].into(),
        )
    }

    #[test]
    fn allows_confident_benign_login() {
        let dec = checker().check("alice@corp.com", "login", 0.95);
        assert!(dec.allowed, "{}", dec.reason);
        assert!(dec.policies_evaluated.contains(&"policy_engine_triager_auto_close".into()));
    }

    #[test]
    fn denies_when_confidence_below_threshold() {
        let dec = checker().check("alice@corp.com", "login", 0.80);
        assert!(!dec.allowed);
        assert!(dec.reason.contains("confidence"), "{}", dec.reason);
    }

    #[test]
    fn denies_principal_on_do_not_touch() {
        let dec = checker().check("ceo@corp.com", "login", 0.99);
        assert!(!dec.allowed);
        assert!(dec.reason.contains("do-not-touch"), "{}", dec.reason);
    }

    #[test]
    fn denies_when_tenant_automation_off() {
        let c = ShadowChecker::new(
            HashSet::new(), 0.90, false, ["login".into()].into(),
        );
        let dec = c.check("alice@corp.com", "login", 0.95);
        assert!(!dec.allowed);
        assert!(dec.reason.contains("tenant"), "{}", dec.reason);
    }

    #[test]
    fn denies_non_allowlisted_action_class() {
        let dec = checker().check("alice@corp.com", "data_exfiltration", 0.99);
        assert!(!dec.allowed);
        assert!(dec.reason.contains("action class"), "{}", dec.reason);
    }
}
