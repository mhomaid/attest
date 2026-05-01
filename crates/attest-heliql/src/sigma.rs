//! Sigma rule compatibility module.
//!
//! Converts a subset of [Sigma](https://github.com/SigmaHQ/sigma) rules (YAML)
//! into HELIQL [`Detection`] AST nodes.  Only the fields used by the 10 bundled
//! MVP detections are supported.  Unknown Sigma fields are silently ignored
//! rather than erroring, so real Sigma files can be imported incrementally.

use crate::ast::*;
use crate::error::HeliqlError;
use serde::Deserialize;
use std::collections::HashMap;

// ── Sigma YAML schema (minimal subset) ──────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SigmaRule {
    title: Option<String>,
    description: Option<String>,
    status: Option<String>,
    level: Option<String>,
    tags: Option<Vec<String>>,
    logsource: Option<SigmaLogSource>,
    detection: Option<SigmaDetection>,
}

#[derive(Debug, Deserialize)]
struct SigmaLogSource {
    product: Option<String>,
    service: Option<String>,
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SigmaDetection {
    #[serde(flatten)]
    selections: HashMap<String, serde_json::Value>,
    condition: Option<String>,
}

// ── Conversion ───────────────────────────────────────────────────────────────

/// Parse a Sigma YAML string and convert it to a HELIQL [`Detection`].
///
/// The `id` parameter overrides the detection ID; if not supplied the Sigma
/// `title` (slugified) is used.
pub fn sigma_to_detection(yaml: &str, id_override: Option<&str>) -> Result<Detection, HeliqlError> {
    let sigma: SigmaRule =
        serde_yaml::from_str(yaml).map_err(|e| HeliqlError::SigmaConversionError(e.to_string()))?;

    let id = id_override
        .map(|s| s.to_string())
        .or_else(|| sigma.title.as_deref().map(slugify))
        .unwrap_or_else(|| "sigma_rule".to_string());

    let severity = sigma
        .level
        .as_deref()
        .map(map_sigma_level)
        .unwrap_or(Severity::Medium);

    let condition = sigma
        .detection
        .as_ref()
        .and_then(|d| d.condition.as_deref());
    let description = merge_sigma_meta(
        sigma.description.clone(),
        sigma.status.as_deref(),
        condition,
    );

    let mitre = sigma
        .tags
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|t| {
            let t = t.strip_prefix("attack.")?;
            if t.starts_with('t') || t.starts_with('T') {
                Some(t.to_uppercase())
            } else {
                None
            }
        })
        .collect();

    let applies_to = sigma.logsource.as_ref().and_then(|ls| {
        match (ls.product.as_deref(), ls.service.as_deref()) {
            (Some("aws"), Some("cloudtrail")) => Some("ocsf.authentication".to_string()),
            (Some("okta"), _) => Some("ocsf.authentication".to_string()),
            (Some("azure") | Some("m365"), _) => Some("ocsf.authentication".to_string()),
            _ if ls
                .category
                .as_deref()
                .is_some_and(|c| c.eq_ignore_ascii_case("authentication")) =>
            {
                Some("ocsf.authentication".to_string())
            }
            _ => None,
        }
    });

    // Convert Sigma detection selections into HELIQL ConditionAtoms
    let mut where_conditions: Vec<Condition> = Vec::new();
    if let Some(det) = &sigma.detection {
        for (sel_name, sel_value) in &det.selections {
            if sel_name == "condition" {
                continue;
            }
            if let Some(atoms) = sigma_selection_to_atoms(sel_value) {
                match atoms.len() {
                    0 => {}
                    1 => {
                        where_conditions.push(Condition::Single(atoms.into_iter().next().unwrap()))
                    }
                    _ => where_conditions.push(Condition::And(atoms)),
                }
            }
        }
    }

    Ok(Detection {
        id,
        description,
        applies_to,
        entity: Some("identity.user".to_string()),
        where_conditions,
        conditions: Vec::new(),
        severity,
        mitre,
        runtime: vec![Runtime::Stream],
    })
}

fn merge_sigma_meta(
    base: Option<String>,
    status: Option<&str>,
    condition: Option<&str>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(d) = base.filter(|s| !s.trim().is_empty()) {
        parts.push(d);
    }
    if let Some(s) = status.filter(|s| !s.trim().is_empty()) {
        parts.push(format!("Sigma status: {s}"));
    }
    if let Some(c) = condition.filter(|s| !s.trim().is_empty()) {
        parts.push(format!("Sigma condition: {c}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn sigma_selection_to_atoms(val: &serde_json::Value) -> Option<Vec<ConditionAtom>> {
    let obj = val.as_object()?;
    let mut atoms = Vec::new();
    for (field, condition) in obj {
        let heliql_field = sigma_field_to_heliql(field);
        match condition {
            serde_json::Value::String(s) => {
                atoms.push(ConditionAtom::Cmp {
                    field: heliql_field,
                    op: CmpOp::Eq,
                    value: Value::Str(s.clone()),
                });
            }
            serde_json::Value::Array(arr) => {
                let values: Vec<Value> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| Value::Str(s.to_string())))
                    .collect();
                if !values.is_empty() {
                    atoms.push(ConditionAtom::In {
                        field: heliql_field,
                        rhs: InRhs::List(values),
                    });
                }
            }
            _ => {}
        }
    }
    Some(atoms)
}

fn sigma_field_to_heliql(field: &str) -> String {
    match field {
        "eventName" | "EventName" => "event.api_operation".into(),
        "eventSource" => "event.api_service".into(),
        "awsRegion" | "AwsRegion" => "event.cloud_region".into(),
        "userAgent" => "user_agent".into(),
        "sourceIPAddress" => "source_ip".into(),
        "userIdentity.type" => "identity.type".into(),
        "userIdentity.userName" => "identity.user.name".into(),
        "requestParameters.bucketName" => "request.bucket_name".into(),
        other => other.to_lowercase().replace('.', "_"),
    }
}

fn map_sigma_level(level: &str) -> Severity {
    match level {
        "critical" => Severity::Critical,
        "high" => Severity::High,
        "medium" => Severity::Medium,
        "low" => Severity::Low,
        _ => Severity::Info,
    }
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
