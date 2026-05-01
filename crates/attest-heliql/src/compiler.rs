//! Compile a [`Detection`] into a RisingWave streaming SQL statement.
//!
//! The output is a `CREATE MATERIALIZED VIEW IF NOT EXISTS` DDL that, when
//! executed against RisingWave, continuously emits rows whenever the detection
//! condition fires.  The detection runtime picks those rows up and forwards
//! them to the `alerts` Redpanda topic.
//!
//! # Column mapping
//! HELIQL field references use dotted names (`event.cloud_region`).  These are
//! mapped to the flat column names of the `cloudtrail_events` source table
//! (created in Phase 1) via [`field_to_column`].

use crate::ast::*;
use crate::error::HeliqlError;

/// Column mapping: HELIQL dotted field → flat SQL column name.
fn field_to_column(field: &str) -> String {
    match field {
        "event.cloud_region" | "cloud_region" => "cloud_region".into(),
        "event.cloud_account" | "cloud_account_uid" => "cloud_account_uid".into(),
        "event.severity" | "severity" => "severity".into(),
        "event.auth_status" | "event.outcome" | "auth_status" => "auth_status".into(),
        "event.api_operation" | "event.activity" | "api_operation" => "api_operation".into(),
        "event.api_service" | "api_service" => "api_service".into(),
        "event.class" | "class_uid" => "class_uid".into(),
        "event.time" | "time" => "\"time\"".into(),
        "identity.user" | "identity.user.name" | "actor_user_name" => "actor_user_name".into(),
        "event.source.country" => "cloud_region".into(),
        "event.cloud_provider" => "'aws'".into(),
        other => other.replace('.', "_"),
    }
}

/// Format a Duration as a Postgres INTERVAL literal (e.g. "90 days", "10 minutes").
fn duration_to_interval(d: &crate::ast::Duration) -> String {
    use crate::ast::DurationUnit;
    match d.unit {
        DurationUnit::Days => format!("{} days", d.value),
        DurationUnit::Hours => format!("{} hours", d.value),
        DurationUnit::Minutes => format!("{} minutes", d.value),
        DurationUnit::Seconds => format!("{} seconds", d.value),
    }
}

fn value_to_sql(v: &Value) -> String {
    match v {
        Value::Str(s) => format!("'{s}'"),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Field(f) => field_to_column(f),
    }
}

/// Compile a single [`ConditionAtom`] into a SQL `WHERE` predicate fragment.
fn atom_to_sql(atom: &ConditionAtom) -> Result<String, HeliqlError> {
    match atom {
        ConditionAtom::Cmp { field, op, value } => {
            let col = field_to_column(field);
            Ok(format!("{col} {op} {}", value_to_sql(value)))
        }

        ConditionAtom::In { field, rhs } => {
            let col = field_to_column(field);
            match rhs {
                InRhs::List(values) => {
                    let list = values
                        .iter()
                        .map(value_to_sql)
                        .collect::<Vec<_>>()
                        .join(", ");
                    Ok(format!("{col} IN ({list})"))
                }
                // baseline(entity, window) — use a self-join to avoid entity_baselines
                // self-masking (the current event would update entity_baselines first).
                // EXISTS means the current field value WAS seen before.
                InRhs::Baseline(b) => {
                    let entity_col = field_to_column(&b.field);
                    let interval = duration_to_interval(&b.window);
                    Ok(format!(
                        "EXISTS (\
                            SELECT 1 FROM cloudtrail_events prior \
                            WHERE prior.{entity_col} = e.{entity_col} \
                            AND prior.{col} = e.{col} \
                            AND prior.event_id <> e.event_id \
                            AND prior.\"time\" >= e.\"time\" - INTERVAL '{interval}'\
                        )"
                    ))
                }
            }
        }

        ConditionAtom::NotIn { field, rhs } => {
            let col = field_to_column(field);
            match rhs {
                InRhs::List(values) => {
                    let list = values
                        .iter()
                        .map(value_to_sql)
                        .collect::<Vec<_>>()
                        .join(", ");
                    Ok(format!("{col} NOT IN ({list})"))
                }
                // NOT IN baseline → region was NOT seen before → anomaly
                InRhs::Baseline(b) => {
                    let entity_col = field_to_column(&b.field);
                    let interval = duration_to_interval(&b.window);
                    Ok(format!(
                        "NOT EXISTS (\
                            SELECT 1 FROM cloudtrail_events prior \
                            WHERE prior.{entity_col} = e.{entity_col} \
                            AND prior.{col} = e.{col} \
                            AND prior.event_id <> e.event_id \
                            AND prior.\"time\" >= e.\"time\" - INTERVAL '{interval}'\
                        )"
                    ))
                }
            }
        }

        ConditionAtom::Unique {
            field,
            window,
            op,
            threshold,
        } => {
            let col = field_to_column(field);
            let interval = window.to_interval_sql();
            // Rewrite as: (SELECT count(DISTINCT field) FROM cloudtrail_events
            //              WHERE actor_user_name = e.actor_user_name
            //              AND time >= NOW() - INTERVAL 'Xd') OP threshold
            Ok(format!(
                "(\
                    SELECT count(DISTINCT {col}) \
                    FROM cloudtrail_events c2 \
                    WHERE c2.actor_user_name = e.actor_user_name \
                    AND c2.\"time\" >= NOW() - {interval}\
                ) {op} {threshold}"
            ))
        }
    }
}

fn condition_to_sql(cond: &Condition) -> Result<String, HeliqlError> {
    match cond {
        Condition::Single(atom) => atom_to_sql(atom),
        Condition::And(atoms) => {
            let parts = atoms
                .iter()
                .map(atom_to_sql)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("({})", parts.join(" AND ")))
        }
        Condition::Or(atoms) => {
            let parts = atoms
                .iter()
                .map(atom_to_sql)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("({})", parts.join(" OR ")))
        }
    }
}

/// Compile a [`Detection`] into a RisingWave `CREATE MATERIALIZED VIEW` DDL.
///
/// The view columns are:
/// - `detection_id`    VARCHAR
/// - `event_id`        VARCHAR
/// - `actor_user_name` VARCHAR
/// - `cloud_region`    VARCHAR
/// - `severity`        VARCHAR  (from the detection, not the event)
/// - `fired_at`        TIMESTAMPTZ
pub fn compile_to_risingwave(d: &Detection) -> Result<String, HeliqlError> {
    let view_name = format!("det_{}", d.id.replace('-', "_"));

    let mut predicates: Vec<String> = Vec::new();

    // where: clauses (pre-filter)
    for cond in &d.where_conditions {
        predicates.push(condition_to_sql(cond)?);
    }
    // condition: clauses (detection logic)
    for cond in &d.conditions {
        predicates.push(condition_to_sql(cond)?);
    }

    let where_sql = if predicates.is_empty() {
        "TRUE".to_string()
    } else {
        predicates.join("\n    AND ")
    };

    let severity_str = d.severity.to_string();

    // RisingWave disallows NOW() in the SELECT list of streaming views.
    // Use the event timestamp (e.time) as fired_at instead.
    let sql = format!(
        r#"CREATE MATERIALIZED VIEW IF NOT EXISTS {view_name} AS
SELECT
    '{det_id}'          AS detection_id,
    e.event_id          AS event_id,
    e.actor_user_name   AS actor_user_name,
    e.cloud_region      AS cloud_region,
    '{severity_str}'    AS severity,
    CAST(e."time" AS VARCHAR) AS fired_at
FROM cloudtrail_events e
WHERE
    {where_sql};"#,
        det_id = d.id,
    );

    Ok(sql)
}

/// Compile a detection to a DROP statement (for teardown/re-deploy).
pub fn drop_view_sql(detection_id: &str) -> String {
    let view_name = format!("det_{}", detection_id.replace('-', "_"));
    format!("DROP MATERIALIZED VIEW IF EXISTS {view_name};")
}
