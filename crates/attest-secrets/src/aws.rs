use async_trait::async_trait;
use tokio::sync::OnceCell;

use crate::{validate_name, SecretError, SecretSource, SecretValue};

/// AWS Secrets Manager. Secret id is `<prefix><name>`. Credentials come from the default
/// AWS chain (IRSA in Kubernetes); nothing is read from Attest config. Wrap in
/// [`crate::CachedSource`] to bound API calls.
pub struct AwsSecretsManagerSource {
    prefix: String,
    client: OnceCell<aws_sdk_secretsmanager::Client>,
}

impl AwsSecretsManagerSource {
    /// Client is created on first use.
    pub fn lazy(prefix: String) -> Self {
        Self {
            prefix,
            client: OnceCell::new(),
        }
    }

    async fn client(&self) -> &aws_sdk_secretsmanager::Client {
        self.client
            .get_or_init(|| async {
                let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
                aws_sdk_secretsmanager::Client::new(&cfg)
            })
            .await
    }
}

#[async_trait]
impl SecretSource for AwsSecretsManagerSource {
    async fn get(&self, name: &str) -> Result<SecretValue, SecretError> {
        validate_name(name)?;
        let out = self
            .client()
            .await
            .get_secret_value()
            .secret_id(format!("{}{}", self.prefix, name))
            .send()
            .await
            .map_err(|e| {
                let svc = e.into_service_error();
                if svc.is_resource_not_found_exception() {
                    SecretError::NotFound(name.to_string())
                } else {
                    SecretError::Backend {
                        name: name.to_string(),
                        message: "get_secret_value failed".into(),
                    }
                }
            })?;
        match out.secret_string() {
            Some(s) if !s.is_empty() => Ok(SecretValue::new(s)),
            _ => Err(SecretError::Empty(name.to_string())),
        }
    }

    fn kind(&self) -> &'static str {
        "aws"
    }
}
