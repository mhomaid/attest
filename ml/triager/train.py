#!/usr/bin/env python3
"""
Train the Triager XGBoost classifier and export to ONNX.

Inputs:
  - golden_cases.json  (200 synthetic labeled cases)

Outputs (in artifacts/):
  - model.onnx             XGBoost binary classifier in ONNX format
  - shap_background.npy    Background dataset for SHAP TreeExplainer (numpy array)
  - feature_hash.txt       SHA-256 of the feature column order (used by Rust for attestation)
  - model_metrics.json     Accuracy, precision, recall, AUC on held-out validation set
"""

import json
import hashlib
import os
import numpy as np
import pandas as pd
from pathlib import Path
from sklearn.model_selection import train_test_split
from sklearn.metrics import accuracy_score, precision_score, recall_score, roc_auc_score
import xgboost as xgb
import onnxmltools
from onnxmltools.convert.common.data_types import FloatTensorType
import onnx

ARTIFACTS = Path(__file__).parent / "artifacts"
ARTIFACTS.mkdir(exist_ok=True)

FEATURE_COLUMNS = [
    "severity_score",
    "source_class_id",
    "entity_reputation_score",
    "baseline_deviation",
    "threat_intel_hit_count",
    "hour_of_day",
    "asset_criticality",
    "prior_disposition_ratio",
]


def load_cases() -> pd.DataFrame:
    cases_path = Path(__file__).parent / "golden_cases.json"
    with open(cases_path) as f:
        data = json.load(f)
    rows = []
    for c in data["cases"]:
        row = {k: v for k, v in c["features"].items()}
        row["label"] = c["label"]
        row["is_ood"] = c["is_ood"]
        rows.append(row)
    return pd.DataFrame(rows)


def main():
    df = load_cases()
    # Exclude OOD cases from training (they are used for novelty detector only)
    train_df = df[~df["is_ood"]].copy()

    X = train_df[FEATURE_COLUMNS].astype(np.float32).values  # numpy array → numeric feature names f0..f7
    y = train_df["label"].astype(np.int32).values

    X_train, X_val, y_train, y_val = train_test_split(X, y, test_size=0.2, random_state=42, stratify=y)

    model = xgb.XGBClassifier(
        n_estimators=100,
        max_depth=4,
        learning_rate=0.1,
        subsample=0.8,
        colsample_bytree=0.8,
        eval_metric="logloss",
        random_state=42,
    )
    model.fit(X_train, y_train, eval_set=[(X_val, y_val)], verbose=False)

    # Metrics
    y_prob = model.predict_proba(X_val)[:, 1]
    y_pred = model.predict(X_val)
    metrics = {
        "accuracy": float(accuracy_score(y_val, y_pred)),
        "precision": float(precision_score(y_val, y_pred, zero_division=0)),
        "recall": float(recall_score(y_val, y_pred, zero_division=0)),
        "auc": float(roc_auc_score(y_val, y_prob)),
        "train_size": len(X_train),
        "val_size": len(X_val),
    }
    print(f"Validation metrics: {metrics}")

    # Export to ONNX
    initial_type = [("float_input", FloatTensorType([None, len(FEATURE_COLUMNS)]))]
    onnx_model = onnxmltools.convert_xgboost(model, initial_types=initial_type)
    onnx_path = ARTIFACTS / "model.onnx"
    onnxmltools.utils.save_model(onnx_model, str(onnx_path))
    print(f"Saved ONNX model → {onnx_path}")

    # Save background dataset for SHAP (sample of training data)
    rng = np.random.RandomState(42)
    bg_idx = rng.choice(len(X_train), size=min(50, len(X_train)), replace=False)
    background = X_train[bg_idx].astype(np.float32)
    np.save(ARTIFACTS / "shap_background.npy", background)

    # Feature column hash (consumed by Rust for attestation envelope)
    col_str = ",".join(FEATURE_COLUMNS)
    feature_hash = hashlib.sha256(col_str.encode()).hexdigest()
    (ARTIFACTS / "feature_hash.txt").write_text(feature_hash)
    print(f"Feature hash: {feature_hash}")

    # Save metrics
    with open(ARTIFACTS / "model_metrics.json", "w") as f:
        json.dump(metrics, f, indent=2)

    # Save model SHA-256 for attestation
    model_bytes = onnx_path.read_bytes()
    model_hash = hashlib.sha256(model_bytes).hexdigest()
    (ARTIFACTS / "model_hash.txt").write_text(model_hash)
    print(f"Model SHA-256: {model_hash}")

    print("Training complete.")


if __name__ == "__main__":
    main()
