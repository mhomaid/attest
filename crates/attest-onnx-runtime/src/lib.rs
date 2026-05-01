//! ONNX Runtime wrapper for the Triager classifier and novelty detector.
//!
//! - `OnnxClassifier` — runs the XGBoost model exported via `ml/triager/train.py`
//!   and approximates SHAP values via a signed-contribution heuristic from the
//!   SHAP background dataset.
//! - `NoveltyDetector` — computes Mahalanobis distance from the mean/inverse-covariance
//!   parameters saved by `ml/triager/novelty.py`. Pure Rust, no ONNX needed.

pub mod classifier;
pub mod novelty;

pub use classifier::OnnxClassifier;
pub use novelty::NoveltyDetector;
