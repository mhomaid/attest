//! User baselines computed from supplied rows.
//!
//! Geography and login hours come only from those rows. There is no built-in
//! country list or business-hours range.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct BaselineRow {
    pub principal: String,
    pub countries: Vec<String>,
    pub hours: Vec<u8>,
    pub logins: u64,
}

pub fn respond(principal: &str, path: Option<&Path>, demo_mode: bool) -> Result<Value, String> {
    let Some(path) = path else {
        if demo_mode {
            return Ok(empty_baseline(principal, "user_baseline_stub"));
        }
        return Err("user baselines are not configured".into());
    };
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("user baselines unreadable: {e}"))?;
    let rows = parse_rows(&text)?;
    Ok(from_rows(principal, &rows))
}

pub fn from_rows(principal: &str, rows: &[BaselineRow]) -> Value {
    let matched: Vec<&BaselineRow> = rows.iter().filter(|r| r.principal == principal).collect();
    if matched.is_empty() {
        return empty_baseline(principal, "user_baseline");
    }
    let mut countries = BTreeSet::new();
    let mut hours = BTreeSet::new();
    let mut login_sum = 0u64;
    for row in &matched {
        countries.extend(row.countries.iter().cloned());
        hours.extend(row.hours.iter().copied());
        login_sum += row.logins;
    }
    let avg = login_sum as f64 / matched.len() as f64;
    json!({
        "principal": principal,
        "avg_daily_logins": avg,
        "typical_geo_countries": countries.into_iter().collect::<Vec<_>>(),
        "typical_login_hours": hours.into_iter().collect::<Vec<_>>(),
        "sample_count": matched.len(),
        "source": "user_baseline"
    })
}

pub fn parse_rows(text: &str) -> Result<Vec<BaselineRow>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("user baselines are not json: {e}"))?;
    let rows = if let Some(arr) = value.as_array() {
        arr
    } else if let Some(arr) = value.get("rows").and_then(|v| v.as_array()) {
        arr
    } else {
        return Err("user baselines must be a JSON array or an object with rows".into());
    };
    let mut parsed = Vec::with_capacity(rows.len());
    for row in rows {
        let principal = string_field(row, &["principal", "actor_user_name", "user"]);
        let Some(principal) = principal else {
            continue;
        };
        let mut countries = Vec::new();
        if let Some(country) = string_field(row, &["country", "region", "geo"]) {
            countries.push(country);
        }
        if let Some(list) = row.get("countries").and_then(|v| v.as_array()) {
            for item in list {
                if let Some(country) = item.as_str() {
                    if !country.is_empty() {
                        countries.push(country.to_string());
                    }
                }
            }
        }
        let mut hours = Vec::new();
        if let Some(hour) = u8_field(row, &["hour", "login_hour"]) {
            hours.push(hour);
        }
        if let Some(list) = row.get("hours").and_then(|v| v.as_array()) {
            for item in list {
                if let Some(hour) = item.as_u64().and_then(|n| u8::try_from(n).ok()) {
                    if hour <= 23 {
                        hours.push(hour);
                    }
                }
            }
        }
        let logins = u64_field(row, &["logins", "event_count"]).unwrap_or(1);
        parsed.push(BaselineRow {
            principal,
            countries,
            hours,
            logins,
        });
    }
    Ok(parsed)
}

fn empty_baseline(principal: &str, source: &str) -> Value {
    json!({
        "principal": principal,
        "avg_daily_logins": 0,
        "typical_geo_countries": [],
        "typical_login_hours": [],
        "sample_count": 0,
        "source": source
    })
}

fn string_field(row: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = row.get(*key).and_then(|v| v.as_str()) {
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn u64_field(row: &Value, keys: &[&str]) -> Option<u64> {
    for key in keys {
        if let Some(n) = row.get(*key).and_then(|v| v.as_u64()) {
            return Some(n);
        }
    }
    None
}

fn u8_field(row: &Value, keys: &[&str]) -> Option<u8> {
    let n = u64_field(row, keys)?;
    let hour = u8::try_from(n).ok()?;
    if hour <= 23 {
        Some(hour)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geography_and_hours_come_from_rows() {
        let rows = parse_rows(
            r#"[
                {"principal":"user-1001","country":"DE","hour":22,"logins":4},
                {"principal":"user-1001","country":"FR","hour":23,"logins":1}
            ]"#,
        )
        .unwrap();
        let value = from_rows("user-1001", &rows);
        assert_eq!(value["typical_geo_countries"], json!(["DE", "FR"]));
        assert_eq!(value["typical_login_hours"], json!([22, 23]));
        assert_eq!(value["avg_daily_logins"], json!(2.5));
        assert_eq!(value["source"], "user_baseline");
        assert!(!crate::demo::is_stub_source(
            value["source"].as_str().unwrap()
        ));
        let hours = value["typical_login_hours"].as_array().unwrap();
        assert!(!hours.contains(&json!(8)));
        assert!(!hours.contains(&json!(9)));
        assert!(!value.to_string().contains("\"US\""));
    }

    #[test]
    fn unknown_principal_is_empty_not_a_default_country() {
        let rows = parse_rows(r#"[{"principal":"user-1001","country":"DE","hour":22}]"#).unwrap();
        let value = from_rows("user-9999", &rows);
        assert_eq!(value["typical_geo_countries"], json!([]));
        assert_eq!(value["typical_login_hours"], json!([]));
        assert_eq!(value["avg_daily_logins"], 0);
        assert_eq!(value["source"], "user_baseline");
    }

    #[test]
    fn wrapped_rows_and_aliases_are_accepted() {
        let rows = parse_rows(
            r#"{"rows":[{"actor_user_name":"user-1001","region":"DE","login_hour":4,"event_count":2}]}"#,
        )
        .unwrap();
        let value = from_rows("user-1001", &rows);
        assert_eq!(value["typical_geo_countries"], json!(["DE"]));
        assert_eq!(value["typical_login_hours"], json!([4]));
        assert_eq!(value["avg_daily_logins"], 2.0);
    }

    #[test]
    fn production_without_a_file_is_not_a_stub() {
        let err = respond("user-1001", None, false).unwrap_err();
        assert!(!err.contains("_stub"), "{err}");
        let stub = respond("user-1001", None, true).unwrap();
        assert!(crate::demo::is_stub_source(
            stub["source"].as_str().unwrap()
        ));
        assert_eq!(stub["typical_geo_countries"], json!([]));
        assert_eq!(stub["typical_login_hours"], json!([]));
    }
}
