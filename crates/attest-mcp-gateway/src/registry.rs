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
    pub fn new() -> Self { Self::default() }

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
        description: "Query ClickHouse over Iceberg warm-tier data (historical events >24h).".into(),
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
        description: "Look up an IP, domain, or file hash in the Attest threat intelligence index.".into(),
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
        description: "Return asset metadata for a given hostname or IP (criticality, owner, environment).".into(),
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
        description: "Return the UEBA baseline statistics for a user principal from RisingWave.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "principal": { "type": "string", "description": "User principal name or ID" }
            },
            "required": ["principal"]
        }),
        class: ToolClass::Internal,
    });

    r
}
