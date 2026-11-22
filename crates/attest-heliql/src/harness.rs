//! In-memory HELIQL evaluator for fixture sequences.
//!
//! This does not start RisingWave. It applies the same predicates the compiler
//! emits: comparisons and lists on the current event, and `unique` / `count` /
//! `baseline` over the entity's events inside the window.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::*;
use crate::error::HeliqlError;
use crate::parser;

/// One synthetic event. `time` is unix seconds. Field names are the flat
/// columns the compiler uses (`actor_user_name`, `auth_status`, `api_service`).
#[derive(Debug, Clone)]
pub struct FixtureEvent {
    pub time: i64,
    pub fields: BTreeMap<String, String>,
}

/// Indexes of events that satisfy the rule.
pub fn evaluate(detection: &Detection, events: &[FixtureEvent]) -> Result<Vec<usize>, HeliqlError> {
    let mut fired = Vec::new();
    for index in 0..events.len() {
        if event_matches(detection, events, index)? {
            fired.push(index);
        }
    }
    Ok(fired)
}

/// Load a JSON array of events. Each object has `time` (RFC3339) plus columns.
pub fn events_from_json(bytes: &str) -> Result<Vec<FixtureEvent>, HeliqlError> {
    let value: serde_json::Value = serde_json::from_str(bytes)
        .map_err(|e| HeliqlError::ParseError(format!("fixture json: {e}")))?;
    let rows = value
        .as_array()
        .ok_or_else(|| HeliqlError::ParseError("fixture must be a JSON array".into()))?;
    rows.iter()
        .map(|row| {
            let obj = row
                .as_object()
                .ok_or_else(|| HeliqlError::ParseError("fixture event must be an object".into()))?;
            let time = obj
                .get("time")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HeliqlError::ParseError("fixture event needs time".into()))?;
            let time = chrono::DateTime::parse_from_rfc3339(time)
                .map_err(|e| HeliqlError::ParseError(format!("fixture time: {e}")))?
                .timestamp();
            let mut fields = BTreeMap::new();
            for (key, value) in obj {
                if key == "time" {
                    continue;
                }
                let text = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    other => other.to_string(),
                };
                fields.insert(key.clone(), text);
            }
            Ok(FixtureEvent { time, fields })
        })
        .collect()
}

pub fn parse_and_evaluate(rule: &str, events: &[FixtureEvent]) -> Result<Vec<usize>, HeliqlError> {
    let detections = parser::parse(rule)?;
    let detection = detections
        .into_iter()
        .next()
        .ok_or_else(|| HeliqlError::ParseError("rule file is empty".into()))?;
    evaluate(&detection, events)
}

fn event_matches(
    detection: &Detection,
    events: &[FixtureEvent],
    index: usize,
) -> Result<bool, HeliqlError> {
    for cond in detection
        .where_conditions
        .iter()
        .chain(detection.conditions.iter())
    {
        if !condition_matches(cond, detection, events, index)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn condition_matches(
    cond: &Condition,
    detection: &Detection,
    events: &[FixtureEvent],
    index: usize,
) -> Result<bool, HeliqlError> {
    match cond {
        Condition::Single(atom) => atom_matches(atom, detection, events, index),
        Condition::And(atoms) => {
            for atom in atoms {
                if !atom_matches(atom, detection, events, index)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Condition::Or(atoms) => {
            for atom in atoms {
                if atom_matches(atom, detection, events, index)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

fn atom_matches(
    atom: &ConditionAtom,
    detection: &Detection,
    events: &[FixtureEvent],
    index: usize,
) -> Result<bool, HeliqlError> {
    let current = &events[index];
    match atom {
        ConditionAtom::Cmp { field, op, value } => {
            let left = field_value(current, field);
            let right = match value {
                Value::Str(s) => s.clone(),
                Value::Int(n) => n.to_string(),
                Value::Float(n) => n.to_string(),
                Value::Field(name) => field_value(current, name),
            };
            Ok(compare(&left, op, &right))
        }
        ConditionAtom::In { field, rhs } => {
            in_list(current, detection, events, index, field, rhs, false)
        }
        ConditionAtom::NotIn { field, rhs } => {
            in_list(current, detection, events, index, field, rhs, true)
        }
        ConditionAtom::Unique {
            field,
            window,
            op,
            threshold,
        } => {
            let distinct = distinct_in_window(detection, events, index, field, window.to_seconds());
            Ok(compare(&distinct.to_string(), op, &threshold.to_string()))
        }
        ConditionAtom::Count {
            window,
            op,
            threshold,
        } => {
            let n = count_in_window(detection, events, index, window.to_seconds())?;
            Ok(compare(&n.to_string(), op, &threshold.to_string()))
        }
    }
}

fn in_list(
    current: &FixtureEvent,
    _detection: &Detection,
    events: &[FixtureEvent],
    index: usize,
    field: &str,
    rhs: &InRhs,
    negate: bool,
) -> Result<bool, HeliqlError> {
    let hit = match rhs {
        InRhs::List(values) => {
            let left = field_value(current, field);
            values.iter().any(|value| match value {
                Value::Str(s) => s == &left,
                Value::Int(n) => n.to_string() == left,
                Value::Float(n) => n.to_string() == left,
                Value::Field(name) => field_value(current, name) == left,
            })
        }
        InRhs::Baseline(baseline) => {
            let value = field_value(current, field);
            let start = current.time - baseline.window.to_seconds();
            let entity_now = field_value(current, &baseline.field);
            events.iter().enumerate().any(|(i, prior)| {
                i != index
                    && prior.time >= start
                    && prior.time <= current.time
                    && field_value(prior, &baseline.field) == entity_now
                    && field_value(prior, field) == value
            })
        }
    };
    Ok(if negate { !hit } else { hit })
}

fn count_in_window(
    detection: &Detection,
    events: &[FixtureEvent],
    index: usize,
    window_secs: i64,
) -> Result<i64, HeliqlError> {
    let current = &events[index];
    let entity = entity_value(detection, current);
    let start = current.time - window_secs;
    let mut n = 0i64;
    for (i, event) in events.iter().enumerate() {
        if event.time < start || event.time > current.time {
            continue;
        }
        if entity_value(detection, event) != entity {
            continue;
        }
        if row_filters_match(detection, events, i)? {
            n += 1;
        }
    }
    Ok(n)
}

fn row_filters_match(
    detection: &Detection,
    events: &[FixtureEvent],
    index: usize,
) -> Result<bool, HeliqlError> {
    for atom in all_atoms(detection) {
        match atom {
            ConditionAtom::Count { .. } | ConditionAtom::Unique { .. } => {}
            ConditionAtom::In {
                rhs: InRhs::Baseline(_),
                ..
            }
            | ConditionAtom::NotIn {
                rhs: InRhs::Baseline(_),
                ..
            } => {}
            other => {
                if !atom_matches(other, detection, events, index)? {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn distinct_in_window(
    detection: &Detection,
    events: &[FixtureEvent],
    index: usize,
    field: &str,
    window_secs: i64,
) -> i64 {
    let current = &events[index];
    let entity = entity_value(detection, current);
    let start = current.time - window_secs;
    let mut seen = BTreeSet::new();
    for event in events {
        if event.time < start || event.time > current.time {
            continue;
        }
        if entity_value(detection, event) != entity {
            continue;
        }
        seen.insert(field_value(event, field));
    }
    seen.len() as i64
}

fn all_atoms(detection: &Detection) -> Vec<&ConditionAtom> {
    let mut out = Vec::new();
    for cond in detection
        .where_conditions
        .iter()
        .chain(detection.conditions.iter())
    {
        match cond {
            Condition::Single(atom) => out.push(atom),
            Condition::And(atoms) | Condition::Or(atoms) => out.extend(atoms),
        }
    }
    out
}

fn entity_value(detection: &Detection, event: &FixtureEvent) -> String {
    let field = detection.entity.as_deref().unwrap_or("identity.user");
    field_value(event, field)
}

fn field_value(event: &FixtureEvent, field: &str) -> String {
    let key = match field {
        "event.cloud_region" | "cloud_region" => "cloud_region",
        "event.cloud_account" | "cloud_account_uid" => "cloud_account_uid",
        "event.severity" | "severity" => "severity",
        "event.auth_status" | "event.outcome" | "auth_status" => "auth_status",
        "event.api_operation" | "event.activity" | "api_operation" => "api_operation",
        "event.api_service" | "api_service" => "api_service",
        "event.class" | "class_uid" => "class_uid",
        "identity.user" | "identity.user.name" | "actor_user_name" => "actor_user_name",
        "event.time" | "time" => "time",
        other => return event.fields.get(other).cloned().unwrap_or_default(),
    };
    event.fields.get(key).cloned().unwrap_or_default()
}

fn compare(left: &str, op: &CmpOp, right: &str) -> bool {
    if let (Ok(l), Ok(r)) = (left.parse::<i64>(), right.parse::<i64>()) {
        return match op {
            CmpOp::Eq => l == r,
            CmpOp::Ne => l != r,
            CmpOp::Gt => l > r,
            CmpOp::Lt => l < r,
            CmpOp::Gte => l >= r,
            CmpOp::Lte => l <= r,
        };
    }
    match op {
        CmpOp::Eq => left == right,
        CmpOp::Ne => left != right,
        CmpOp::Gt => left > right,
        CmpOp::Lt => left < right,
        CmpOp::Gte => left >= right,
        CmpOp::Lte => left <= right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failures(user: &str, n: usize, start: &str) -> String {
        let start = chrono::DateTime::parse_from_rfc3339(start).unwrap();
        let rows: Vec<serde_json::Value> = (0..n)
            .map(|i| {
                let time = (start + chrono::Duration::seconds(i as i64)).to_rfc3339();
                serde_json::json!({
                    "time": time,
                    "actor_user_name": user,
                    "api_service": "okta",
                    "auth_status": "Failure"
                })
            })
            .collect();
        serde_json::to_string(&rows).unwrap()
    }

    #[test]
    fn brute_force_fires_at_threshold_and_not_one_below() {
        let rule = include_str!("../../../detections/okta_brute_force.heliql");
        let positive = events_from_json(&failures("ada", 5, "2026-01-01T00:00:00Z")).unwrap();
        let negative = events_from_json(&failures("ada", 4, "2026-01-01T00:00:00Z")).unwrap();
        assert!(!parse_and_evaluate(rule, &positive).unwrap().is_empty());
        assert!(parse_and_evaluate(rule, &negative).unwrap().is_empty());
    }

    #[test]
    fn every_shipped_rule_has_a_passing_positive_and_negative_fixture() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../detections");
        let mut rules = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("heliql"))
            .collect::<Vec<_>>();
        rules.sort();
        assert!(!rules.is_empty(), "no rules in detections/");
        for path in rules {
            let source = std::fs::read_to_string(&path).unwrap();
            let detection = parser::parse(&source).unwrap().remove(0);
            let fixture_dir = root.join("fixtures");
            let positive = fixture_dir.join(format!("{}.positive.json", detection.id));
            let negative = fixture_dir.join(format!("{}.negative.json", detection.id));
            let pos_events = events_from_json(
                &std::fs::read_to_string(&positive)
                    .unwrap_or_else(|e| panic!("missing {}: {e}", positive.display())),
            )
            .unwrap();
            let neg_events = events_from_json(
                &std::fs::read_to_string(&negative)
                    .unwrap_or_else(|e| panic!("missing {}: {e}", negative.display())),
            )
            .unwrap();
            assert!(
                !evaluate(&detection, &pos_events).unwrap().is_empty(),
                "{} positive fixture did not fire",
                detection.id
            );
            assert!(
                evaluate(&detection, &neg_events).unwrap().is_empty(),
                "{} negative fixture fired",
                detection.id
            );
        }
    }
}
