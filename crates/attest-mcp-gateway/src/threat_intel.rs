//! Local threat-intel lookup.
//!
//! The feed is a JSON file already on disk (a synthetic abuse.ch-shaped export
//! is enough). Lookups do not download ThreatFox, URLhaus, or MISP.

use std::path::Path;

use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct Indicator {
    value: String,
    kind: String,
    feed: String,
    malware: String,
    confidence: Option<u32>,
    reference: String,
}

#[derive(Debug, Clone)]
pub struct Feed {
    last_sync: String,
    feeds: Vec<String>,
    indicators: Vec<Indicator>,
}

pub fn respond(
    indicator: &str,
    indicator_type: Option<&str>,
    feed_path: Option<&Path>,
    demo_mode: bool,
) -> Result<Value, String> {
    let Some(path) = feed_path else {
        if demo_mode {
            return Ok(json!({
                "indicator": indicator,
                "indicator_type": indicator_type,
                "malicious": false,
                "confidence": 0.0,
                "matches": [],
                "last_sync": null,
                "source": "threat_intel_stub"
            }));
        }
        return Err("threat intel feed is not configured".into());
    };
    lookup_file(path, indicator, indicator_type)
}

pub fn lookup_file(
    path: &Path,
    indicator: &str,
    indicator_type: Option<&str>,
) -> Result<Value, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("threat intel feed unreadable: {e}"))?;
    let fallback = file_mtime_rfc3339(path);
    let feed = parse_feed(&text, &fallback)?;
    Ok(lookup(&feed, indicator, indicator_type))
}

pub fn parse_feed(text: &str, fallback_sync: &str) -> Result<Feed, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("threat intel feed is not json: {e}"))?;
    let last_sync = value
        .get("synced_at")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback_sync)
        .to_string();
    let mut feeds = Vec::new();
    let mut indicators = Vec::new();
    if let Some(fox) = value.get("threatfox").or_else(|| {
        if value.get("data").is_some() {
            Some(&value)
        } else {
            None
        }
    }) {
        if fox.get("data").is_some() {
            feeds.push("threatfox".into());
            push_threatfox(fox, &mut indicators)?;
        }
    }
    if let Some(haus) = value.get("urlhaus") {
        feeds.push("urlhaus".into());
        push_urlhaus(haus, &mut indicators)?;
    }
    if let Some(misp) = value.get("misp") {
        feeds.push("misp".into());
        push_misp(misp, &mut indicators)?;
    }
    if feeds.is_empty() {
        return Err("threat intel feed has no threatfox, urlhaus, or misp entries".into());
    }
    Ok(Feed {
        last_sync,
        feeds,
        indicators,
    })
}

pub fn lookup(feed: &Feed, indicator: &str, indicator_type: Option<&str>) -> Value {
    let needle = indicator.trim();
    let matches: Vec<Value> = feed
        .indicators
        .iter()
        .filter(|item| indicator_matches(item, needle))
        .map(|item| {
            json!({
                "indicator": item.value,
                "type": item.kind,
                "feed": item.feed,
                "malware": item.malware,
                "confidence": item.confidence,
                "reference": item.reference
            })
        })
        .collect();
    let confidence = matches
        .iter()
        .filter_map(|m| m.get("confidence").and_then(|v| v.as_u64()))
        .max()
        .map(|n| n as f64 / 100.0)
        .unwrap_or(if matches.is_empty() { 0.0 } else { 0.5 });
    json!({
        "indicator": needle,
        "indicator_type": indicator_type,
        "malicious": !matches.is_empty(),
        "confidence": confidence,
        "matches": matches,
        "last_sync": feed.last_sync,
        "feeds": feed.feeds,
        "source": "threat_intel_feed"
    })
}

fn push_threatfox(fox: &Value, out: &mut Vec<Indicator>) -> Result<(), String> {
    let rows = fox
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "threatfox feed is missing data".to_string())?;
    for row in rows {
        let value = row
            .get("ioc")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if value.is_empty() {
            continue;
        }
        out.push(Indicator {
            value,
            kind: row
                .get("ioc_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            feed: "threatfox".into(),
            malware: row
                .get("malware")
                .or_else(|| row.get("threat_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            confidence: row
                .get("confidence_level")
                .and_then(|v| v.as_u64())
                .and_then(|n| u32::try_from(n).ok()),
            reference: row
                .get("reference")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        });
    }
    Ok(())
}

fn push_urlhaus(haus: &Value, out: &mut Vec<Indicator>) -> Result<(), String> {
    let rows = haus
        .get("urls")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "urlhaus feed is missing urls".to_string())?;
    for row in rows {
        let url = row.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let host = row.get("host").and_then(|v| v.as_str()).unwrap_or("");
        if !url.is_empty() {
            out.push(urlhaus_indicator(row, url, "url"));
        }
        if !host.is_empty() {
            out.push(urlhaus_indicator(row, host, "domain"));
        }
    }
    Ok(())
}

fn urlhaus_indicator(row: &Value, value: &str, kind: &str) -> Indicator {
    Indicator {
        value: value.to_string(),
        kind: kind.into(),
        feed: "urlhaus".into(),
        malware: row
            .get("threat")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        confidence: None,
        reference: row
            .get("urlhaus_reference")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

fn push_misp(misp: &Value, out: &mut Vec<Indicator>) -> Result<(), String> {
    let rows = misp
        .as_array()
        .ok_or_else(|| "misp feed must be an array of attributes".to_string())?;
    for row in rows {
        let value = row
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if value.is_empty() {
            continue;
        }
        out.push(Indicator {
            value,
            kind: row
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("misp")
                .to_string(),
            feed: "misp".into(),
            malware: row
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            confidence: None,
            reference: String::new(),
        });
    }
    Ok(())
}

fn indicator_matches(item: &Indicator, needle: &str) -> bool {
    if item.value.eq_ignore_ascii_case(needle) {
        return true;
    }
    if let Some((host, _port)) = item.value.rsplit_once(':') {
        if host.eq_ignore_ascii_case(needle) && !host.contains(':') {
            return true;
        }
    }
    false
}

fn file_mtime_rfc3339(path: &Path) -> String {
    let Ok(meta) = std::fs::metadata(path) else {
        return String::new();
    };
    let Ok(modified) = meta.modified() else {
        return String::new();
    };
    let dt: chrono::DateTime<chrono::Utc> = modified.into();
    dt.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> &'static str {
        r#"{
            "synced_at": "2026-01-15T00:00:00Z",
            "threatfox": {
                "query_status": "ok",
                "data": [
                    {
                        "ioc": "203.0.113.10:443",
                        "ioc_type": "ip:port",
                        "threat_type": "botnet_cc",
                        "malware": "synthetic-loader",
                        "confidence_level": 75,
                        "reference": "https://example.com/ioc/1001"
                    },
                    {
                        "ioc": "evil.example.com",
                        "ioc_type": "domain",
                        "threat_type": "payload_delivery",
                        "malware": "synthetic-loader",
                        "confidence_level": 60
                    }
                ]
            },
            "urlhaus": {
                "query_status": "ok",
                "urls": [
                    {
                        "id": "1001",
                        "url": "https://example.com/payload",
                        "host": "example.com",
                        "threat": "malware_download",
                        "url_status": "online",
                        "urlhaus_reference": "https://example.com/url/1001"
                    }
                ]
            }
        }"#
    }

    #[test]
    fn threatfox_and_urlhaus_lookup_includes_last_sync() {
        let feed = parse_feed(sample(), "fallback").unwrap();
        let hit = lookup(&feed, "203.0.113.10", Some("ip"));
        assert_eq!(hit["malicious"], true);
        assert_eq!(hit["last_sync"], "2026-01-15T00:00:00Z");
        assert_eq!(hit["source"], "threat_intel_feed");
        assert!(!crate::demo::is_stub_source(
            hit["source"].as_str().unwrap()
        ));
        assert_eq!(hit["matches"][0]["feed"], "threatfox");
        assert_eq!(hit["matches"][0]["malware"], "synthetic-loader");
        assert!((hit["confidence"].as_f64().unwrap() - 0.75).abs() < f64::EPSILON);
        let domain = lookup(&feed, "example.com", Some("domain"));
        assert_eq!(domain["malicious"], true);
        assert_eq!(domain["matches"][0]["feed"], "urlhaus");
        let miss = lookup(&feed, "198.51.100.8", Some("ip"));
        assert_eq!(miss["malicious"], false);
        assert_eq!(miss["last_sync"], "2026-01-15T00:00:00Z");
        assert_eq!(
            lookup(&feed, "notexample.com", Some("domain"))["malicious"],
            false
        );
    }

    #[test]
    fn lookup_reads_only_the_local_file() {
        let dir = std::env::temp_dir().join(format!("attest-intel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("feed.json");
        let body = r#"{"threatfox":{"query_status":"ok","data":[{"ioc":"203.0.113.10","ioc_type":"ip","malware":"synthetic-loader","confidence_level":10}]}}"#;
        std::fs::write(&path, body).unwrap();
        let hit = lookup_file(&path, "203.0.113.10", Some("ip")).unwrap();
        assert_eq!(hit["source"], "threat_intel_feed");
        assert!(!hit["last_sync"].as_str().unwrap().is_empty());
        assert_ne!(hit["last_sync"], "fallback");
        let err =
            lookup_file(dir.join("missing.json").as_path(), "203.0.113.10", None).unwrap_err();
        assert!(err.contains("unreadable"), "{err}");
        assert!(!err.to_ascii_lowercase().contains("http"));
    }

    #[test]
    fn misp_is_optional() {
        let without = parse_feed(sample(), "fallback").unwrap();
        assert!(!without.feeds.iter().any(|f| f == "misp"));
        let with = r#"{
            "synced_at": "2026-02-01T00:00:00Z",
            "misp": [{"value": "203.0.113.50", "type": "ip-dst", "category": "Network activity"}]
        }"#;
        let feed = parse_feed(with, "fallback").unwrap();
        let hit = lookup(&feed, "203.0.113.50", Some("ip"));
        assert_eq!(hit["malicious"], true);
        assert_eq!(hit["matches"][0]["feed"], "misp");
        assert_eq!(hit["last_sync"], "2026-02-01T00:00:00Z");
    }

    #[test]
    fn production_without_a_file_is_not_a_stub() {
        let err = respond("203.0.113.10", Some("ip"), None, false).unwrap_err();
        assert!(!err.contains("_stub"), "{err}");
        let stub = respond("203.0.113.10", Some("ip"), None, true).unwrap();
        assert!(crate::demo::is_stub_source(
            stub["source"].as_str().unwrap()
        ));
    }
}
