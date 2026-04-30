//! Calibration sidecar client.
//!
//! Calls the Python Flask sidecar at `CALIBRATION_URL` (default http://localhost:5001).
//! Phase 4b adds an optional `path` tag ("classifier" | "llm") so the sidecar can
//! maintain per-path calibration models. The sidecar falls back to the per-class
//! model when no per-path model is trained yet.

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct CalibrationRequest<'a> {
    agent_id: &'a str,
    case_class: &'a str,
    raw_score: f32,
    /// Execution path tag for per-path calibration models.
    path: &'a str,
}

#[derive(Deserialize)]
struct CalibrationResponse {
    calibrated_score: f32,
}

pub struct CalibrationClient {
    base_url: String,
    client: reqwest::Client,
}

impl CalibrationClient {
    pub fn from_env() -> Self {
        let base_url = std::env::var("CALIBRATION_URL")
            .unwrap_or_else(|_| "http://localhost:5001".into());
        Self { base_url, client: reqwest::Client::new() }
    }

    /// Request a calibrated confidence score from the sidecar.
    ///
    /// `path` is one of `"classifier"` | `"llm"` — the sidecar uses it to
    /// select a per-path model when one is trained, and falls back to the
    /// per-class model otherwise.
    ///
    /// Falls back to `raw_score` if the sidecar is unavailable.
    pub async fn calibrate(&self, agent_id: &str, case_class: &str, raw_score: f32, path: &str) -> f32 {
        match self.try_calibrate(agent_id, case_class, raw_score, path).await {
            Ok(score) => score,
            Err(e) => {
                tracing::warn!(error = %e, raw_score, path, "calibration sidecar unavailable — using raw score");
                raw_score
            }
        }
    }

    async fn try_calibrate(&self, agent_id: &str, case_class: &str, raw_score: f32, path: &str) -> Result<f32> {
        let url = format!("{}/calibrate", self.base_url);
        let resp: CalibrationResponse = self.client
            .post(&url)
            .json(&CalibrationRequest { agent_id, case_class, raw_score, path })
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.calibrated_score)
    }
}
