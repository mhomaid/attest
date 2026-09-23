//! Feature extractor — converts an OCSF alert/event into the tabular feature
//! vector consumed by the XGBoost/ONNX classifier.
//!
//! The 8 features here match the column order in `ml/triager/train.py` exactly.
//! If you add or reorder features you MUST retrain the model.

use attest_common::ocsf::{OcsfEvent, Severity};
use chrono::Timelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tabular feature vector for the Triager XGBoost classifier.
/// All values are f64 for direct ONNX input tensor construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertFeatures {
    /// Severity normalised to 0–1.
    pub severity_score: f64,

    /// Integer source class id from OCSF (e.g. 3002=Auth, 6003=CloudAPI).
    pub source_class_id: f64,

    /// Entity reputation from threat intel stub (0=clean, 1=known-bad).
    pub entity_reputation_score: f64,

    /// Z-score deviation from the entity's RisingWave baseline (0 if no baseline).
    pub baseline_deviation: f64,

    /// Number of matching threat intel indicators in the event.
    pub threat_intel_hit_count: f64,

    /// Hour of day the event occurred (0–23).
    pub hour_of_day: f64,

    /// Asset criticality (0=unknown, 0.5=standard, 1.0=production-critical).
    pub asset_criticality: f64,

    /// Fraction of historically-similar alerts that were true-positives (0–1).
    pub prior_disposition_ratio: f64,
}

impl AlertFeatures {
    /// Column names in the exact order the ONNX model expects them.
    pub const COLUMN_ORDER: &'static [&'static str] = &[
        "severity_score",
        "source_class_id",
        "entity_reputation_score",
        "baseline_deviation",
        "threat_intel_hit_count",
        "hour_of_day",
        "asset_criticality",
        "prior_disposition_ratio",
    ];

    /// Serialise into a flat f32 slice matching `COLUMN_ORDER` for ONNX input.
    pub fn to_vec_f32(&self) -> Vec<f32> {
        vec![
            self.severity_score as f32,
            self.source_class_id as f32,
            self.entity_reputation_score as f32,
            self.baseline_deviation as f32,
            self.threat_intel_hit_count as f32,
            self.hour_of_day as f32,
            self.asset_criticality as f32,
            self.prior_disposition_ratio as f32,
        ]
    }

    /// Named map for SHAP attribution display.
    pub fn to_named_map(&self) -> HashMap<String, f64> {
        let mut m = HashMap::new();
        m.insert("severity_score".into(), self.severity_score);
        m.insert("source_class_id".into(), self.source_class_id);
        m.insert(
            "entity_reputation_score".into(),
            self.entity_reputation_score,
        );
        m.insert("baseline_deviation".into(), self.baseline_deviation);
        m.insert("threat_intel_hit_count".into(), self.threat_intel_hit_count);
        m.insert("hour_of_day".into(), self.hour_of_day);
        m.insert("asset_criticality".into(), self.asset_criticality);
        m.insert(
            "prior_disposition_ratio".into(),
            self.prior_disposition_ratio,
        );
        m
    }

    /// Rebuild the vector from a named map (as stored on `ClassifierEvidence`).
    /// Every column in `COLUMN_ORDER` must be present.
    pub fn from_named_map(map: &HashMap<String, f64>) -> Result<Self, String> {
        let get = |name: &str| {
            map.get(name)
                .copied()
                .ok_or_else(|| format!("missing feature `{name}`"))
        };
        Ok(Self {
            severity_score: get("severity_score")?,
            source_class_id: get("source_class_id")?,
            entity_reputation_score: get("entity_reputation_score")?,
            baseline_deviation: get("baseline_deviation")?,
            threat_intel_hit_count: get("threat_intel_hit_count")?,
            hour_of_day: get("hour_of_day")?,
            asset_criticality: get("asset_criticality")?,
            prior_disposition_ratio: get("prior_disposition_ratio")?,
        })
    }

    /// SHA-256 of the comma-joined column names — matches `ml/triager/artifacts/feature_hash.txt`.
    pub fn column_order_hash() -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(Self::COLUMN_ORDER.join(",").as_bytes()))
    }
}

/// Extracts `AlertFeatures` from an OCSF event.
pub struct FeatureExtractor;

impl FeatureExtractor {
    pub fn extract(event: &OcsfEvent) -> AlertFeatures {
        let hour = event.time().hour() as f64;
        let (severity_score, source_class_id) = match event {
            OcsfEvent::Authentication(e) => (severity_to_score(&e.severity), 3002.0),
            OcsfEvent::CloudActivity(e) => (severity_to_score(&e.severity), 6003.0),
        };

        AlertFeatures {
            severity_score,
            source_class_id,
            entity_reputation_score: 0.0,
            baseline_deviation: 0.0,
            threat_intel_hit_count: 0.0,
            hour_of_day: hour,
            asset_criticality: 0.5,
            prior_disposition_ratio: 0.5,
        }
    }

    /// Build features from a raw JSON alert payload (used by the orchestrator HTTP API).
    ///
    /// Accepts two formats transparently:
    /// 1. **Pre-computed** – JSON already contains `severity_score`, `source_class_id`, etc.
    /// 2. **OCSF flat** – JSON contains `severity` (string), `class_uid` (string), `time`
    ///    (ISO-8601 string) as produced by the collector.  Fields missing from either format
    ///    fall back to safe defaults.
    pub fn extract_from_json(alert: &serde_json::Value) -> AlertFeatures {
        // ── severity_score ────────────────────────────────────────────────────
        let severity_score = alert["severity_score"].as_f64().unwrap_or_else(|| {
            match alert["severity"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .as_str()
            {
                "informational" => 0.1,
                "low" => 0.3,
                "medium" => 0.5,
                "high" => 0.8,
                "critical" | "fatal" => 1.0,
                _ => 0.5,
            }
        });

        // ── source_class_id ──────────────────────────────────────────────────
        let source_class_id = alert["source_class_id"].as_f64().unwrap_or_else(|| {
            alert["class_uid"]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(6003.0)
        });

        // ── hour_of_day ───────────────────────────────────────────────────────
        // Prefer an explicit `hour_of_day` field; otherwise parse `time` / `eventTime`.
        let hour_of_day = alert["hour_of_day"].as_f64().unwrap_or_else(|| {
            let ts = alert["time"]
                .as_str()
                .or_else(|| alert["eventTime"].as_str())
                .unwrap_or("");
            // Take the HH component from "YYYY-MM-DDTHH:…" or "YYYY-MM-DD HH:…"
            ts.chars()
                .skip(11)
                .take(2)
                .collect::<String>()
                .parse::<f64>()
                .unwrap_or(12.0)
        });

        AlertFeatures {
            severity_score,
            source_class_id,
            entity_reputation_score: alert["entity_reputation_score"].as_f64().unwrap_or(0.0),
            baseline_deviation: alert["baseline_deviation"].as_f64().unwrap_or(0.0),
            threat_intel_hit_count: alert["threat_intel_hit_count"].as_f64().unwrap_or(0.0),
            hour_of_day,
            asset_criticality: alert["asset_criticality"].as_f64().unwrap_or(0.5),
            prior_disposition_ratio: alert["prior_disposition_ratio"].as_f64().unwrap_or(0.5),
        }
    }
}

fn severity_to_score(severity: &Severity) -> f64 {
    match severity {
        Severity::Unknown => 0.0,
        Severity::Informational => 0.1,
        Severity::Low => 0.3,
        Severity::Medium => 0.5,
        Severity::High => 0.8,
        Severity::Critical | Severity::Fatal => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_from_json_defaults() {
        let f = FeatureExtractor::extract_from_json(&serde_json::json!({}));
        assert_eq!(f.severity_score, 0.5);
        assert_eq!(f.source_class_id, 6003.0);
        assert_eq!(AlertFeatures::COLUMN_ORDER.len(), 8);
    }

    #[test]
    fn to_vec_f32_length() {
        let f = FeatureExtractor::extract_from_json(&serde_json::json!({"severity_score": 0.8}));
        assert_eq!(f.to_vec_f32().len(), 8);
    }

    #[test]
    fn named_map_has_all_columns() {
        let f = FeatureExtractor::extract_from_json(&serde_json::json!({}));
        let m = f.to_named_map();
        for col in AlertFeatures::COLUMN_ORDER {
            assert!(m.contains_key(*col), "missing column: {col}");
        }
        let back = AlertFeatures::from_named_map(&m).unwrap();
        assert_eq!(back.to_vec_f32(), f.to_vec_f32());
        assert!(AlertFeatures::from_named_map(&HashMap::new()).is_err());
    }

    #[test]
    fn column_order_hash_matches_committed_artifact() {
        let expected = include_str!("../../../ml/triager/artifacts/feature_hash.txt").trim();
        assert_eq!(AlertFeatures::column_order_hash(), expected);
    }
}
