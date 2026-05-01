//! Mahalanobis-distance novelty (OOD) detector.
//!
//! Loads the mean vector and inverse-covariance matrix saved by
//! `ml/triager/novelty.py` and computes Mahalanobis distance in pure Rust.
//! The distance is normalised to [0, 1] via a sigmoid over the learned threshold.

use anyhow::{Context, Result};
use attest_feature_extractor::AlertFeatures;
use std::path::Path;

pub struct NoveltyDetector {
    mean: Vec<f64>,
    inv_cov: Vec<Vec<f64>>, // [n_features × n_features]
    /// 95th-percentile distance on training data; scores above → OOD.
    threshold: f64,
    n_features: usize,
}

impl NoveltyDetector {
    /// Load novelty parameters from numpy files.
    ///
    /// `mean_path`     — `artifacts/novelty_mean.npy`
    /// `inv_cov_path`  — `artifacts/novelty_inv_cov.npy`
    /// `threshold_path`— `artifacts/novelty_threshold.txt`
    pub fn load(
        mean_path: impl AsRef<Path>,
        inv_cov_path: impl AsRef<Path>,
        threshold_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let mean = load_npy_1d_f64(mean_path.as_ref())?;
        let n_features = mean.len();
        let inv_cov_flat = load_npy_1d_f64(inv_cov_path.as_ref())?;

        anyhow::ensure!(
            inv_cov_flat.len() == n_features * n_features,
            "inv_cov has unexpected size {} (expected {}×{})",
            inv_cov_flat.len(), n_features, n_features
        );

        let inv_cov: Vec<Vec<f64>> = inv_cov_flat
            .chunks(n_features)
            .map(|row| row.to_vec())
            .collect();

        let threshold_str = std::fs::read_to_string(threshold_path.as_ref())
            .context("failed to read novelty_threshold.txt")?;
        let threshold: f64 = threshold_str.trim().parse()
            .context("failed to parse novelty threshold")?;

        Ok(Self { mean, inv_cov, threshold, n_features })
    }

    /// Return a novelty score in [0, 1].
    ///
    /// Score < 0.5 → in-distribution; Score > 0.5 → out-of-distribution.
    /// Uses a sigmoid centred on the training threshold.
    pub fn score(&self, features: &AlertFeatures) -> f32 {
        let feat = features.to_vec_f32();
        let dist = self.mahalanobis_distance(&feat);
        // sigmoid((dist - threshold) / (threshold * 0.3))
        let k = if self.threshold > 0.0 { self.threshold * 0.3 } else { 1.0 };
        let logit = (dist - self.threshold) / k;
        let sigmoid = 1.0 / (1.0 + (-logit).exp());
        sigmoid as f32
    }

    /// Return true if the sample is out-of-distribution (distance > threshold).
    pub fn is_ood(&self, features: &AlertFeatures) -> bool {
        let feat = features.to_vec_f32();
        self.mahalanobis_distance(&feat) > self.threshold
    }

    fn mahalanobis_distance(&self, feat: &[f32]) -> f64 {
        let n = self.n_features;
        let diff: Vec<f64> = (0..n)
            .map(|i| feat.get(i).copied().unwrap_or(0.0) as f64 - self.mean[i])
            .collect();

        // tmp = diff @ inv_cov
        let mut tmp = vec![0.0f64; n];
        for (j, tmp_j) in tmp.iter_mut().enumerate() {
            for (i, diff_i) in diff.iter().enumerate() {
                *tmp_j += diff_i * self.inv_cov[i][j];
            }
        }

        // dist² = tmp · diff
        let dist_sq: f64 = tmp.iter().zip(diff.iter()).map(|(a, b)| a * b).sum();
        dist_sq.max(0.0).sqrt()
    }
}

/// Parse a 1D or 2D numpy `.npy` file and return all values as f64.
fn load_npy_1d_f64(path: &Path) -> Result<Vec<f64>> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read npy file: {}", path.display()))?;

    anyhow::ensure!(bytes.starts_with(b"\x93NUMPY"), "not a numpy file");

    let header_len = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    let data_offset = 10 + header_len;
    let data_bytes = &bytes[data_offset..];

    let header = std::str::from_utf8(&bytes[10..data_offset])
        .unwrap_or("")
        .to_lowercase();

    let is_f64 = header.contains("float64") || header.contains("'<f8'") || header.contains("\"<f8\"");
    let elem_size = if is_f64 { 8 } else { 4 };

    let mut values = Vec::with_capacity(data_bytes.len() / elem_size);
    for chunk in data_bytes.chunks(elem_size) {
        if chunk.len() < elem_size { break; }
        let v = if is_f64 {
            f64::from_le_bytes(chunk.try_into().unwrap())
        } else {
            f32::from_le_bytes(chunk.try_into().unwrap()) as f64
        };
        values.push(v);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use attest_feature_extractor::FeatureExtractor;

    #[test]
    fn mahalanobis_on_identity_covariance() {
        // With identity inverse covariance and zero mean, distance = L2 norm.
        let n = 8;
        let mean = vec![0.0f64; n];
        let mut inv_cov = vec![vec![0.0f64; n]; n];
        for (i, row) in inv_cov.iter_mut().enumerate().take(n) {
            row[i] = 1.0;
        }
        let det = NoveltyDetector { mean, inv_cov, threshold: 3.0, n_features: n };
        let features = FeatureExtractor::extract_from_json(&serde_json::json!({
            "severity_score": 1.0,
            "source_class_id": 0.0,
            "entity_reputation_score": 0.0,
            "baseline_deviation": 0.0,
            "threat_intel_hit_count": 0.0,
            "hour_of_day": 0.0,
            "asset_criticality": 0.0,
            "prior_disposition_ratio": 0.0,
        }));
        let dist = det.mahalanobis_distance(&features.to_vec_f32());
        assert!((dist - 1.0).abs() < 1e-4, "expected ~1.0 got {dist}");
    }
}
