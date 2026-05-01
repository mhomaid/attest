#!/usr/bin/env python3
"""
Train the Mahalanobis-distance novelty (OOD) detector.

Rather than exporting to ONNX (EllipticEnvelope is not supported by skl2onnx),
we save the fitted parameters as numpy arrays. The Rust onnx-runtime crate
loads these files and computes the Mahalanobis distance in-process.

Outputs (in artifacts/):
  - novelty_mean.npy          Feature mean vector (shape: [8])
  - novelty_inv_cov.npy       Inverse covariance matrix (shape: [8, 8])
  - novelty_threshold.txt     95th-percentile Mahalanobis distance on training set
"""

import json
import hashlib
import numpy as np
from pathlib import Path
from sklearn.covariance import EmpiricalCovariance

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


def load_in_distribution() -> np.ndarray:
    cases_path = Path(__file__).parent / "golden_cases.json"
    with open(cases_path) as f:
        data = json.load(f)
    rows = []
    for c in data["cases"]:
        if not c["is_ood"]:
            rows.append([c["features"][col] for col in FEATURE_COLUMNS])
    return np.array(rows, dtype=np.float64)


def mahalanobis(X: np.ndarray, mean: np.ndarray, inv_cov: np.ndarray) -> np.ndarray:
    diff = X - mean
    return np.sqrt(np.sum(diff @ inv_cov * diff, axis=1))


def main():
    X_in = load_in_distribution()
    print(f"In-distribution training samples: {len(X_in)}")

    cov = EmpiricalCovariance(assume_centered=False)
    cov.fit(X_in)

    mean = cov.location_
    inv_cov_matrix = cov.precision_

    np.save(ARTIFACTS / "novelty_mean.npy", mean)
    np.save(ARTIFACTS / "novelty_inv_cov.npy", inv_cov_matrix)

    # Compute threshold: 95th percentile of training distances
    distances = mahalanobis(X_in, mean, inv_cov_matrix)
    threshold = float(np.percentile(distances, 95))
    (ARTIFACTS / "novelty_threshold.txt").write_text(str(threshold))
    print(f"Novelty threshold (95th pct Mahalanobis distance): {threshold:.4f}")

    # Save SHA-256 of the parameters for attestation
    files_hash = hashlib.sha256(
        np.load(ARTIFACTS / "novelty_mean.npy").tobytes() +
        np.load(ARTIFACTS / "novelty_inv_cov.npy").tobytes()
    ).hexdigest()
    (ARTIFACTS / "novelty_hash.txt").write_text(files_hash)
    print(f"Novelty params SHA-256: {files_hash}")
    print("Novelty detector training complete.")


if __name__ == "__main__":
    main()
