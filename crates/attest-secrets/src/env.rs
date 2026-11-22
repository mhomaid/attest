use async_trait::async_trait;

use crate::{validate_name, SecretError, SecretSource, SecretValue};

/// Reads `<PREFIX><NAME>` from the process environment (name upper-cased, `-` and `.`
/// become `_`). Development only: environment variables leak into child processes and
/// crash dumps. Reads happen on every `get`.
pub struct EnvSource {
    prefix: String,
}

impl EnvSource {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    fn var_name(&self, name: &str) -> String {
        let mapped: String = name
            .chars()
            .map(|c| match c {
                '-' | '.' => '_',
                c => c.to_ascii_uppercase(),
            })
            .collect();
        format!("{}{}", self.prefix, mapped)
    }
}

#[async_trait]
impl SecretSource for EnvSource {
    async fn get(&self, name: &str) -> Result<SecretValue, SecretError> {
        validate_name(name)?;
        match std::env::var(self.var_name(name)) {
            Ok(v) if v.is_empty() => Err(SecretError::Empty(name.to_string())),
            Ok(v) => Ok(SecretValue::new(v)),
            Err(std::env::VarError::NotPresent) => Err(SecretError::NotFound(name.to_string())),
            Err(std::env::VarError::NotUnicode(_)) => Err(SecretError::Backend {
                name: name.to_string(),
                message: "value is not valid UTF-8".into(),
            }),
        }
    }

    fn kind(&self) -> &'static str {
        "env"
    }
}
