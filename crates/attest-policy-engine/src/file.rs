//! Policy file: role allow-lists and a do-not-touch list.
//!
//! The default file is the current rule set. A bad file fails load with a
//! clear error so a process can refuse to start. The file hash is what
//! signed envelopes record.

use sha2::{Digest, Sha256};

use crate::{AgentRole, PolicyContext, PolicyDecision};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PolicyFile {
    #[serde(default)]
    pub read_only: Vec<String>,
    #[serde(default)]
    pub roles: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub do_not_touch: DoNotTouch,
    #[serde(skip)]
    pub raw: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DoNotTouch {
    #[serde(default)]
    pub users: Vec<String>,
    #[serde(default)]
    pub accounts: Vec<String>,
    #[serde(default)]
    pub ips: Vec<String>,
    #[serde(default)]
    pub asns: Vec<String>,
    #[serde(default)]
    pub tenants: Vec<String>,
    /// `deny` or `escalate`. Anything else is a load error.
    #[serde(default = "default_effect")]
    pub effect: String,
}

impl Default for DoNotTouch {
    fn default() -> Self {
        Self {
            users: Vec::new(),
            accounts: Vec::new(),
            ips: Vec::new(),
            asns: Vec::new(),
            tenants: Vec::new(),
            effect: default_effect(),
        }
    }
}

fn default_effect() -> String {
    "deny".into()
}

const BUILTIN: &str = include_str!("../policy.default.yaml");

impl PolicyFile {
    pub fn builtin() -> Self {
        Self::parse(BUILTIN).expect("built-in policy file is valid")
    }

    /// Load the built-in file and record its hash for later envelope signatures.
    pub fn install(&self) {
        attest_attestation::set_policy_hash(self.hash());
    }

    pub fn install_builtin() -> Self {
        let file = Self::builtin();
        file.install();
        file
    }

    pub fn parse(yaml: &str) -> Result<Self, String> {
        let mut file: PolicyFile =
            serde_yaml::from_str(yaml).map_err(|e| format!("invalid policy file: {e}"))?;
        if file.do_not_touch.effect != "deny" && file.do_not_touch.effect != "escalate" {
            return Err(format!(
                "invalid policy file: do_not_touch.effect must be deny or escalate, got {}",
                file.do_not_touch.effect
            ));
        }
        file.raw = yaml.to_string();
        Ok(file)
    }

    pub fn hash(&self) -> String {
        hex::encode(Sha256::digest(self.raw.as_bytes()))
    }

    pub fn allows(&self, role: &AgentRole, tool_id: &str) -> bool {
        if self.read_only.iter().any(|t| t == tool_id) {
            return true;
        }
        let key = match role {
            AgentRole::Triager => "triager",
            AgentRole::Investigator => "investigator",
            AgentRole::Hunter => "hunter",
            AgentRole::DetectionEngineer => "detection_engineer",
            AgentRole::Responder => "responder",
            AgentRole::Coordinator => "coordinator",
        };
        self.roles
            .get(key)
            .map(|tools| tools.iter().any(|t| t == tool_id))
            .unwrap_or(false)
    }
}

/// A listed entity makes every action Deny or Escalate. It never turns a
/// denial into an allow.
pub fn do_not_touch_decision(file: &PolicyFile, ctx: &PolicyContext) -> Option<PolicyDecision> {
    let list = &file.do_not_touch;
    let hit = list.users.iter().any(|v| eq(v, &ctx.user))
        || list.accounts.iter().any(|v| eq(v, &ctx.account))
        || list.ips.iter().any(|v| eq(v, &ctx.ip))
        || list.asns.iter().any(|v| eq(v, &ctx.asn))
        || list.tenants.iter().any(|v| eq(v, &ctx.tenant_id));
    if !hit {
        return None;
    }
    let reason = "entity is on the do-not-touch list";
    Some(if list.effect == "escalate" {
        PolicyDecision::escalate(reason)
    } else {
        PolicyDecision::deny(reason)
    })
}

fn eq(listed: &str, value: &str) -> bool {
    !value.is_empty() && listed.eq_ignore_ascii_case(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_policy_file_fails_with_a_clear_error() {
        let err = PolicyFile::parse("roles: [\n").unwrap_err();
        assert!(err.contains("invalid policy file"), "{err}");
        let err = PolicyFile::parse("do_not_touch:\n  effect: allow\n").unwrap_err();
        assert!(err.contains("deny or escalate"), "{err}");
    }

    #[test]
    fn default_file_lists_the_hard_coded_tools() {
        let file = PolicyFile::builtin();
        assert!(file.allows(&AgentRole::Triager, "query_hot_tier"));
        assert!(file.allows(&AgentRole::Triager, "triager_auto_close"));
        assert!(file.allows(&AgentRole::Investigator, "graylog_search"));
        assert!(!file.allows(&AgentRole::Coordinator, "idp_revoke_session"));
        assert!(!file.allows(&AgentRole::Triager, "edr_isolate_host"));
        assert!(!file.hash().is_empty());
    }

    #[test]
    fn example_overlay_policy_loads() {
        let yaml = include_str!("../../../examples/overlay/policy.yaml");
        let file = PolicyFile::parse(yaml).expect("overlay policy");
        assert!(file.allows(&AgentRole::Investigator, "graylog_search"));
        assert!(file.allows(&AgentRole::Investigator, "ip_asn_lookup"));
        assert!(file.allows(&AgentRole::Investigator, "entity_lookup"));
        let ctx = PolicyContext {
            user: "user-1001".into(),
            ..PolicyContext::default()
        };
        assert!(
            !crate::authorize_with(&file, &AgentRole::Responder, "firewall_block_ioc", &ctx)
                .is_allowed()
        );
        assert!(
            !crate::authorize_with(&file, &AgentRole::Investigator, "ip_asn_lookup", &ctx)
                .is_allowed()
        );
        assert!(
            !crate::authorize_with(&file, &AgentRole::Investigator, "entity_lookup", &ctx)
                .is_allowed()
        );
    }

    #[test]
    fn install_records_the_file_hash_for_envelopes() {
        let file = PolicyFile::install_builtin();
        assert_eq!(
            attest_attestation::installed_policy_hash().as_deref(),
            Some(file.hash().as_str())
        );
    }

    #[test]
    fn do_not_touch_user_is_denied_even_for_a_read_tool() {
        let yaml = r#"
read_only: [query_hot_tier]
do_not_touch:
  users: ["alice"]
  effect: deny
"#;
        let file = PolicyFile::parse(yaml).unwrap();
        let mut ctx = PolicyContext {
            user: "alice".into(),
            ..PolicyContext::default()
        };
        let decision =
            crate::authorize_with(&file, &AgentRole::Investigator, "query_hot_tier", &ctx);
        assert!(!decision.is_allowed());
        ctx.user = "bob".into();
        assert!(do_not_touch_decision(&file, &ctx).is_none());
    }

    #[test]
    fn do_not_touch_can_escalate_instead_of_deny() {
        let yaml = r#"
do_not_touch:
  ips: ["203.0.113.10"]
  asns: ["AS64500"]
  accounts: ["1001"]
  tenants: ["tenant-a"]
  effect: escalate
"#;
        let file = PolicyFile::parse(yaml).unwrap();
        for ctx in [
            PolicyContext {
                ip: "203.0.113.10".into(),
                ..PolicyContext::default()
            },
            PolicyContext {
                asn: "AS64500".into(),
                ..PolicyContext::default()
            },
            PolicyContext {
                account: "1001".into(),
                ..PolicyContext::default()
            },
            PolicyContext {
                tenant_id: "tenant-a".into(),
                ..PolicyContext::default()
            },
        ] {
            assert!(matches!(
                do_not_touch_decision(&file, &ctx),
                Some(PolicyDecision::Escalate { .. })
            ));
        }
    }
}
