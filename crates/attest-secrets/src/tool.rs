use std::sync::Arc;

use async_trait::async_trait;

use crate::{validate_name, SecretError, SecretSource, SecretValue};

/// Resolves a credential for one MCP tool ID. The secret name is `tool-<tool_id>`, so a
/// Kubernetes Secret with key `tool-graylog_search` provides the credential for the tool
/// `graylog_search`. Nothing is cached here; rotation shows up on the next `resolve`.
#[derive(Clone)]
pub struct ToolCredentials {
    source: Arc<dyn SecretSource>,
}

impl ToolCredentials {
    pub fn new(source: Arc<dyn SecretSource>) -> Self {
        Self { source }
    }

    pub fn secret_name(tool_id: &str) -> Result<String, SecretError> {
        validate_name(tool_id)?;
        Ok(format!("tool-{tool_id}"))
    }

    pub async fn resolve(&self, tool_id: &str) -> Result<SecretValue, SecretError> {
        self.source.get(&Self::secret_name(tool_id)?).await
    }

    pub fn source_kind(&self) -> &'static str {
        self.source.kind()
    }
}

#[async_trait]
impl SecretSource for ToolCredentials {
    async fn get(&self, name: &str) -> Result<SecretValue, SecretError> {
        self.source.get(name).await
    }

    fn kind(&self) -> &'static str {
        self.source.kind()
    }
}
