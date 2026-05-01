//! XGBoost ONNX classifier wrapper using tract-onnx (pure Rust, no C++ deps).
//!
//! SHAP values are approximated via marginal-contribution: for each feature,
//! we measure the prediction change when replacing it with the background mean.

use anyhow::{Context, Result};
use attest_feature_extractor::AlertFeatures;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tract_onnx::prelude::*;

pub struct OnnxClassifier {
    model: Arc<TypedRunnableModel>,
    shap_background_means: Vec<f32>,
    num_features: usize,
}

impl OnnxClassifier {
    /// Load the ONNX model.
    ///
    /// `model_path`      — path to `artifacts/model.onnx`
    /// `background_path` — path to `artifacts/shap_background.npy` (optional)
    pub fn load(model_path: impl AsRef<Path>, background_path: Option<&str>) -> Result<Self> {
        let num_features = AlertFeatures::COLUMN_ORDER.len(); // 8

        // Provide explicit input shape: batch=1, features=num_features (f32)
        let model = tract_onnx::onnx()
            .model_for_path(model_path.as_ref())
            .context("failed to load ONNX model")?
            .with_input_fact(
                0,
                InferenceFact::dt_shape(
                    f32::datum_type(),
                    &[1usize, num_features][..],
                ),
            )
            .context("failed to set input fact")?
            .into_optimized()
            .context("failed to optimise ONNX model")?
            .into_runnable()
            .context("failed to make ONNX model runnable")?;

        let shap_background_means = if let Some(bp) = background_path {
            load_npy_means(Path::new(bp), num_features)
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "failed to load SHAP background — using zeros");
                    vec![0.0f32; num_features]
                })
        } else {
            vec![0.0f32; num_features]
        };

        Ok(Self { model, shap_background_means, num_features })
    }

    /// Run inference and return `(raw_score, shap_values_map)`.
    pub fn predict(&self, features: &AlertFeatures) -> Result<(f32, HashMap<String, f64>)> {
        let feat_vec = features.to_vec_f32();
        let raw_score = self.run_model(&feat_vec)?;
        let shap_values = self.approximate_shap(features, raw_score)?;
        Ok((raw_score, shap_values))
    }

    fn run_model(&self, feat_vec: &[f32]) -> Result<f32> {
        // Build input tensor: shape [1, num_features], dtype f32
        let input: Tensor = ndarray::Array2::from_shape_vec(
            (1, self.num_features),
            feat_vec.to_vec(),
        )
        .context("failed to build input array")?
        .into();

        let outputs = self.model
            .run(tvec!(input.into()))
            .context("ONNX inference failed")?;

        // XGBoost ONNX from onnxmltools exports two outputs:
        //   0: label (int64, shape [1])
        //   1: probabilities (shape [1, 2]) — P(neg), P(pos)
        // Try output index 1 first; fall back to index 0 for single-output models.
        let prob_tensor = if outputs.len() >= 2 {
            &outputs[1]
        } else {
            &outputs[0]
        };

        // Extract as f32 view and return P(positive) = column 1
        let view = prob_tensor
            .to_plain_array_view::<f32>()
            .context("probability output is not f32")?;

        let flat: Vec<f32> = view.iter().cloned().collect();
        // flat = [P(neg), P(pos)] for binary classifier
        let positive_prob = if flat.len() >= 2 { flat[1] } else { flat[0] };
        Ok(positive_prob)
    }

    fn approximate_shap(&self, features: &AlertFeatures, baseline_score: f32) -> Result<HashMap<String, f64>> {
        let feat_vec = features.to_vec_f32();
        let mut shap = HashMap::new();
        for (i, name) in AlertFeatures::COLUMN_ORDER.iter().enumerate() {
            let mut perturbed = feat_vec.clone();
            perturbed[i] = self.shap_background_means[i];
            let perturbed_score = self.run_model(&perturbed)?;
            shap.insert(name.to_string(), (baseline_score - perturbed_score) as f64);
        }
        Ok(shap)
    }
}

/// Parse a numpy `.npy` file and return column means (handles 1D and 2D float32/float64).
fn load_npy_means(path: &Path, num_features: usize) -> Result<Vec<f32>> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read npy file: {}", path.display()))?;
    anyhow::ensure!(bytes.starts_with(b"\x93NUMPY"), "not a numpy file");
    let header_len = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    let data_offset = 10 + header_len;
    let data_bytes = &bytes[data_offset..];
    let header = std::str::from_utf8(&bytes[10..data_offset]).unwrap_or("").to_lowercase();
    let is_f64 = header.contains("float64") || header.contains("'<f8'") || header.contains("\"<f8\"");
    let elem_size = if is_f64 { 8 } else { 4 };
    let mut values = Vec::with_capacity(data_bytes.len() / elem_size);
    for chunk in data_bytes.chunks(elem_size) {
        if chunk.len() < elem_size { break; }
        values.push(if is_f64 {
            f64::from_le_bytes(chunk.try_into().unwrap()) as f32
        } else {
            f32::from_le_bytes(chunk.try_into().unwrap())
        });
    }
    if values.len() > num_features && values.len() % num_features == 0 {
        let n_rows = values.len() / num_features;
        let mut means = vec![0.0f32; num_features];
        for row in 0..n_rows {
            for col in 0..num_features {
                means[col] += values[row * num_features + col];
            }
        }
        for m in &mut means { *m /= n_rows as f32; }
        Ok(means)
    } else if values.len() == num_features {
        Ok(values)
    } else {
        tracing::warn!(path = %path.display(), "unexpected npy shape — using zeros");
        Ok(vec![0.0f32; num_features])
    }
}
