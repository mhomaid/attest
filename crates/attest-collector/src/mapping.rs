//! Config-driven field mapping and redaction (T-024).
//!
//! Deployer field names live in the YAML a private overlay supplies. This crate
//! only ships an empty default. Emails are hashed, card-like numbers are dropped,
//! and unmapped keys land under `unmapped.<prefix>`.

use serde::Deserialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::error::CollectorError;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FieldMapping {
    #[serde(default = "default_prefix")]
    pub prefix: String,
    /// Source field path (dot-separated) → OCSF path this collector understands.
    #[serde(default)]
    pub fields: Vec<FieldMap>,
    /// Drop these field names after hashing emails and dropping card-like values.
    #[serde(default)]
    pub drop: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldMap {
    pub from: String,
    pub to: String,
}

fn default_prefix() -> String {
    "ext".into()
}

impl FieldMapping {
    pub fn parse(yaml: &str) -> Result<Self, CollectorError> {
        serde_yaml::from_str(yaml)
            .map_err(|e| CollectorError::Normalization(format!("invalid field mapping: {e}")))
    }

    pub fn load_from_env() -> Self {
        match std::env::var("ATTEST_FIELD_MAP_FILE") {
            Ok(path) => match std::fs::read_to_string(&path) {
                Ok(text) => match Self::parse(&text) {
                    Ok(mapping) => mapping,
                    Err(e) => {
                        tracing::error!(%path, "{e}");
                        Self::builtin()
                    }
                },
                Err(e) => {
                    tracing::error!(%path, "field mapping unreadable: {e}");
                    Self::builtin()
                }
            },
            Err(_) => Self::builtin(),
        }
    }

    pub fn builtin() -> Self {
        Self {
            prefix: "ext".into(),
            fields: Vec::new(),
            drop: vec!["email".into(), "phone".into(), "card".into()],
        }
    }
}

/// Apply mapping and redaction. Returns the rewritten object and the unmapped remainder.
pub fn apply(raw: &Value, mapping: &FieldMapping) -> (Value, Option<Value>) {
    let Some(obj) = raw.as_object() else {
        return (redact_value(raw), None);
    };
    let mut mapped = Map::new();
    let mut unmapped = Map::new();
    for (key, value) in obj {
        let value = redact_value(value);
        if mapping.drop.iter().any(|d| d == key) {
            continue;
        }
        if let Some(rule) = mapping.fields.iter().find(|f| f.from == *key) {
            mapped.insert(rule.to.clone(), value);
        } else {
            mapped.insert(key.clone(), value.clone());
            unmapped.insert(key.clone(), value);
        }
    }
    let unmapped = if unmapped.is_empty() {
        None
    } else {
        Some(serde_json::json!({ &mapping.prefix: Value::Object(unmapped) }))
    };
    (Value::Object(mapped), unmapped)
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::String(s) => Value::String(redact_text(s)),
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                if k.eq_ignore_ascii_case("email") || k.eq_ignore_ascii_case("phone") {
                    if let Some(text) = v.as_str() {
                        out.insert(k.clone(), Value::String(hash_text(text)));
                        continue;
                    }
                }
                if k.eq_ignore_ascii_case("card") {
                    continue;
                }
                out.insert(k.clone(), redact_value(v));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

fn redact_text(input: &str) -> String {
    let mut out = String::new();
    for (n, token) in input.split_whitespace().enumerate() {
        if n > 0 {
            out.push(' ');
        }
        if token.contains('@') && token.contains('.') {
            out.push_str(&hash_text(token.trim_matches(|c: char| {
                !c.is_ascii_alphanumeric() && c != '@' && c != '.'
            })));
        } else if is_card_like(token) {
            out.push_str("[redacted-card]");
        } else {
            out.push_str(token);
        }
    }
    if out.is_empty() {
        input.to_string()
    } else {
        out
    }
}

fn is_card_like(token: &str) -> bool {
    let digits: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
    (13..=19).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit())
}

pub fn hash_text(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_is_hashed_and_card_is_dropped() {
        let raw = serde_json::json!({
            "title": "login failed for alice@example.com",
            "card": "4242424242424242",
            "note": "pan 4242424242424242",
            "ticket": "1001"
        });
        let (mapped, unmapped) = apply(&raw, &FieldMapping::builtin());
        let blob = serde_json::to_string(&mapped).unwrap() + &unmapped.unwrap().to_string();
        assert!(!blob.contains("alice@example.com"));
        assert!(!blob.contains("4242424242424242"));
        assert!(blob.contains("sha256:"));
        assert!(blob.contains("1001"));
        assert!(blob.contains("ext"));
    }

    #[test]
    fn invalid_mapping_yaml_is_an_error() {
        let err = FieldMapping::parse("fields: [").unwrap_err();
        assert!(err.to_string().contains("invalid field mapping"));
    }
}
