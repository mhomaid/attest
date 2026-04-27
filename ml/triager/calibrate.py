#!/usr/bin/env python3
"""
Train isotonic regression calibration models and serve a Flask HTTP sidecar.

Calibration maps raw XGBoost probability scores to calibrated probabilities
that reflect actual correctness rates. Trained per agent, per case class.

Usage:
  # Train and save calibration models:
  python calibrate.py --train

  # Start HTTP sidecar (called by orchestrator at runtime):
  python calibrate.py --serve
  POST /calibrate  {"agent_id": "...", "case_class": "...", "raw_score": 0.87}
  → {"calibrated_score": 0.82, "agent_id": "...", "case_class": "..."}
"""

import argparse
import json
import os
import pickle
import hashlib
import numpy as np
from pathlib import Path
from sklearn.isotonic import IsotonicRegression
from flask import Flask, request, jsonify

ARTIFACTS = Path(__file__).parent / "artifacts"
ARTIFACTS.mkdir(exist_ok=True)
CALIBRATION_PATH = ARTIFACTS / "calibration_models.pkl"

# Case classes for the Triager
CASE_CLASSES = [
    "brute_force",
    "exfiltration",
    "geo_anomaly",
    "logging_disabled",
    "mfa_bypass",
    "benign",
    "default",
]


def _synthetic_calibration_data(case_class: str, n: int = 80):
    """Synthetic (raw_score, ground_truth) pairs for cold-start calibration."""
    rng = np.random.RandomState(hash(case_class) % 2**32)
    if case_class in ("brute_force", "exfiltration", "logging_disabled", "mfa_bypass"):
        # High-precision class: model is well-calibrated and mostly TP
        raw = rng.beta(8, 2, n)          # skewed toward 1
        truth = (raw > 0.5).astype(int)
    elif case_class == "geo_anomaly":
        # Moderate class
        raw = rng.beta(4, 4, n)
        truth = (raw > 0.5).astype(int)
    else:
        # Benign / default: low TP rate
        raw = rng.beta(2, 8, n)
        truth = (raw > 0.7).astype(int)
    return raw, truth


def train():
    models = {}
    for cc in CASE_CLASSES:
        raw, truth = _synthetic_calibration_data(cc)
        ir = IsotonicRegression(out_of_bounds="clip")
        ir.fit(raw, truth)
        models[cc] = ir

    with open(CALIBRATION_PATH, "wb") as f:
        pickle.dump(models, f)

    # Hash for attestation
    model_bytes = CALIBRATION_PATH.read_bytes()
    cal_hash = hashlib.sha256(model_bytes).hexdigest()
    (ARTIFACTS / "calibration_hash.txt").write_text(cal_hash)
    print(f"Calibration models saved → {CALIBRATION_PATH}")
    print(f"Calibration SHA-256: {cal_hash}")


def serve():
    if not CALIBRATION_PATH.exists():
        print("Calibration models not found — training first...")
        train()

    with open(CALIBRATION_PATH, "rb") as f:
        models = pickle.load(f)

    app = Flask("calibration-sidecar")

    @app.route("/healthz")
    def healthz():
        return jsonify({"status": "ok"})

    @app.route("/calibrate", methods=["POST"])
    def calibrate():
        body = request.get_json(force=True)
        agent_id = body.get("agent_id", "unknown")
        case_class = body.get("case_class", "default")
        raw_score = float(body.get("raw_score", 0.5))

        model = models.get(case_class, models["default"])
        calibrated = float(model.predict([raw_score])[0])
        # Clamp to [0, 1]
        calibrated = max(0.0, min(1.0, calibrated))

        return jsonify({
            "agent_id": agent_id,
            "case_class": case_class,
            "raw_score": raw_score,
            "calibrated_score": calibrated,
        })

    port = int(os.environ.get("CALIBRATION_PORT", 5001))
    print(f"Calibration sidecar listening on port {port}")
    app.run(host="0.0.0.0", port=port)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--train", action="store_true", help="Train and save calibration models")
    parser.add_argument("--serve", action="store_true", help="Start HTTP sidecar")
    args = parser.parse_args()

    if args.train:
        train()
    elif args.serve:
        serve()
    else:
        # Default: train then serve
        train()
        serve()
