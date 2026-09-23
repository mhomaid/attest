//! Tool registry — in-memory map of available tools with their schemas.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Describes a tool the LLM can call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub id: String,
    pub description: String,
    /// JSON Schema for the tool's input arguments.
    pub input_schema: Value,
    /// Tool class: internal | external | restricted_action
    pub class: ToolClass,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolClass {
    /// Built into Attest (hot/warm tier, baselines, threat intel).
    Internal,
    /// Customer-approved third-party MCP server (Phase 4b+).
    External,
    /// Destructive action (session revoke, host isolate, etc.).
    RestrictedAction,
}

/// In-memory tool registry populated at gateway startup.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolDescriptor>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: ToolDescriptor) {
        self.tools.insert(tool.id.clone(), tool);
    }

    pub fn get(&self, id: &str) -> Option<&ToolDescriptor> {
        self.tools.get(id)
    }

    pub fn list(&self) -> Vec<&ToolDescriptor> {
        self.tools.values().collect()
    }
}

/// Build the default registry with all 5 internal Attest tools.
pub fn default_registry() -> ToolRegistry {
    let mut r = ToolRegistry::new();

    r.register(ToolDescriptor {
        id: "query_hot_tier".into(),
        description: "Query ClickHouse for recent OCSF events (last 24h by default).".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "sql": { "type": "string", "description": "ClickHouse SQL query" },
                "limit": { "type": "integer", "default": 100 }
            },
            "required": ["sql"]
        }),
        class: ToolClass::Internal,
    });

    r.register(ToolDescriptor {
        id: "query_warm_tier".into(),
        description: "Query ClickHouse over Iceberg warm-tier data (historical events >24h)."
            .into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "sql": { "type": "string" },
                "limit": { "type": "integer", "default": 500 }
            },
            "required": ["sql"]
        }),
        class: ToolClass::Internal,
    });

    r.register(ToolDescriptor {
        id: "lookup_threat_intel".into(),
        description: "Look up an IP, domain, or file hash in the Attest threat intelligence index."
            .into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "indicator": { "type": "string" },
                "indicator_type": { "type": "string", "enum": ["ip", "domain", "file_hash"] }
            },
            "required": ["indicator", "indicator_type"]
        }),
        class: ToolClass::Internal,
    });

    r.register(ToolDescriptor {
        id: "get_asset_context".into(),
        description:
            "Return asset metadata for a given hostname or IP (criticality, owner, environment)."
                .into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "asset": { "type": "string" }
            },
            "required": ["asset"]
        }),
        class: ToolClass::Internal,
    });

    r.register(ToolDescriptor {
        id: "get_user_baseline".into(),
        description: "Return the UEBA baseline statistics for a user principal from RisingWave."
            .into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "principal": { "type": "string", "description": "User principal name or ID" }
            },
            "required": ["principal"]
        }),
        class: ToolClass::Internal,
    });

    r.register(ToolDescriptor {
        id: "analyze_code_snippet".into(),
        description: "Static analysis stub for code snippets referenced in alerts (MVP).".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "snippet_id": { "type": "string" },
                "code": { "type": "string" }
            },
            "required": ["snippet_id"]
        }),
        class: ToolClass::Internal,
    });

    r.register(ToolDescriptor {
        id: "sandbox_detonate".into(),
        description: "Sandbox detonation stub for suspicious payloads (MVP — no live detonation)."
            .into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "artifact_hash": { "type": "string" }
            },
            "required": ["artifact_hash"]
        }),
        class: ToolClass::Internal,
    });

    r.register(ToolDescriptor {
        id: "propose_detection_pr".into(),
        description: "Open a draft HELIQL detection PR (does not merge).".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "heliql": { "type": "string" },
                "rationale": { "type": "string" }
            },
            "required": ["title", "heliql"]
        }),
        class: ToolClass::Internal,
    });

    r.register(ToolDescriptor {
        id: "request_human_review".into(),
        description: "Escalate a finding to a human analyst.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": { "reason": { "type": "string" } },
            "required": ["reason"]
        }),
        class: ToolClass::Internal,
    });

    r.register(ToolDescriptor {
        id: "idp_revoke_session".into(),
        description: "Revoke an IdP session. Restricted; policy-gated.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": { "principal": { "type": "string" } },
            "required": ["principal"]
        }),
        class: ToolClass::RestrictedAction,
    });

    r.register(ToolDescriptor {
        id: "edr_isolate_host".into(),
        description: "Isolate a host via EDR. Restricted; policy-gated.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": { "host": { "type": "string" } },
            "required": ["host"]
        }),
        class: ToolClass::RestrictedAction,
    });

    r.register(ToolDescriptor {
        id: "firewall_block_ioc".into(),
        description: "Block an IOC at the firewall. Restricted; policy-gated.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": { "indicator": { "type": "string" } },
            "required": ["indicator"]
        }),
        class: ToolClass::RestrictedAction,
    });

    r
}
