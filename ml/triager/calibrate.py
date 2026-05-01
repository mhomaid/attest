#!/usr/bin/env python3
"""
Train isotonic regression calibration models and serve a FastAPI HTTP sidecar.

Calibration maps raw XGBoost probability scores to calibrated probabilities
that reflect actual correctness rates. Trained per agent, per case class.

Phase 6 additions:
  POST /feedback  Log ground-truth labels and retrain on demand.
                  Persists Brier score and ECE to artifacts/calibration_metrics.json.

Usage:
  # Train and save calibration models:
  python calibrate.py --train

  # Start HTTP sidecar (called by orchestrator at runtime):
  python calibrate.py --serve
  POST /calibrate  {"agent_id": "...", "case_class": "...", "raw_score": 0.87}
  Response: {"calibrated_score": 0.82, "agent_id": "...", "case_class": "..."}
  POST /feedback   {"case_id": "...", "raw_score": 0.87, "case_class": "brute_force",
                    "ground_truth": 1}
  Response: {"status": "logged"}
"""

import argparse
import json
import os
import pickle
import hashlib
import threading
import numpy as np
from pathlib import Path
from sklearn.isotonic import IsotonicRegression
from fastapi import FastAPI
from fastapi.responses import JSONResponse, Response
from pydantic import BaseModel, ConfigDict
import uvicorn

ARTIFACTS = Path(__file__).parent / "artifacts"
ARTIFACTS.mkdir(exist_ok=True)
CALIBRATION_PATH = ARTIFACTS / "calibration_models.pkl"
FEEDBACK_PATH = ARTIFACTS / "feedback_log.jsonl"
METRICS_PATH = ARTIFACTS / "calibration_metrics.json"

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


# ── Calibration metrics ───────────────────────────────────────────────────────

def brier_score(y_true, y_prob):
    """Mean squared error between calibrated probability and binary ground truth."""
    y_true = np.asarray(y_true, dtype=float)
    y_prob = np.asarray(y_prob, dtype=float)
    return float(np.mean((y_prob - y_true) ** 2))


def expected_calibration_error(y_true, y_prob, n_bins: int = 10):
    """ECE: weighted average absolute difference between confidence and accuracy."""
    y_true = np.asarray(y_true, dtype=float)
    y_prob = np.asarray(y_prob, dtype=float)
    bins = np.linspace(0.0, 1.0, n_bins + 1)
    ece = 0.0
    for i in range(n_bins):
        mask = (y_prob >= bins[i]) & (y_prob < bins[i + 1])
        if mask.sum() == 0:
            continue
        bin_confidence = y_prob[mask].mean()
        bin_accuracy = y_true[mask].mean()
        ece += (mask.sum() / len(y_true)) * abs(bin_confidence - bin_accuracy)
    return float(ece)


def compute_and_save_metrics(models: dict) -> dict:
    """Re-compute Brier score and ECE for each case class and persist to disk."""
    metrics = {}
    for cc in CASE_CLASSES:
        raw, truth = _synthetic_calibration_data(cc)
        model = models.get(cc, models["default"])
        calibrated = model.predict(raw)
        calibrated = np.clip(calibrated, 0.0, 1.0)
        metrics[cc] = {
            "brier_score": brier_score(truth, calibrated),
            "ece": expected_calibration_error(truth, calibrated),
            "n_samples": len(raw),
        }
    METRICS_PATH.write_text(json.dumps(metrics, indent=2))
    print(f"Calibration metrics saved → {METRICS_PATH}")
    return metrics


# ── Training ──────────────────────────────────────────────────────────────────

def train(feedback: list | None = None):
    """Train calibration models, optionally incorporating real feedback labels."""
    models = {}
    for cc in CASE_CLASSES:
        raw, truth = _synthetic_calibration_data(cc)

        # Incorporate real feedback for this case class
        if feedback:
            fb = [(r["raw_score"], r["ground_truth"]) for r in feedback if r.get("case_class") == cc]
            if fb:
                fb_raw, fb_truth = zip(*fb)
                raw = np.concatenate([raw, np.array(fb_raw)])
                truth = np.concatenate([truth, np.array(fb_truth, dtype=int)])

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

    # Compute and persist metrics
    compute_and_save_metrics(models)
    return models


# ── HTTP sidecar ──────────────────────────────────────────────────────────────


class CalibrateIn(BaseModel):
    model_config = ConfigDict(extra="ignore")

    agent_id: str = "unknown"
    case_class: str = "default"
    raw_score: float = 0.5
    # Sent by orchestrator for future per-path models; ignored until trained.
    path: str | None = None


class FeedbackIn(BaseModel):
    model_config = ConfigDict(extra="ignore")

    case_id: str
    raw_score: float
    case_class: str
    ground_truth: int


def serve():
    if not CALIBRATION_PATH.exists():
        print("Calibration models not found — training first...")
        train()

    with open(CALIBRATION_PATH, "rb") as f:
        models = pickle.load(f)

    state = {"models": models}
    feedback_lock = threading.Lock()

    app = FastAPI(title="calibration-sidecar", version="0.1.0")

    @app.get("/healthz")
    def healthz():
        return {"status": "ok"}

    @app.post("/calibrate")
    def calibrate_ep(body: CalibrateIn):
        model = state["models"].get(body.case_class, state["models"]["default"])
        calibrated = float(model.predict([body.raw_score])[0])
        calibrated = max(0.0, min(1.0, calibrated))
        return {
            "agent_id": body.agent_id,
            "case_class": body.case_class,
            "raw_score": body.raw_score,
            "calibrated_score": calibrated,
        }

    @app.post("/feedback")
    def feedback_ep(body: FeedbackIn):
        entry = {
            "case_id": str(body.case_id),
            "raw_score": float(body.raw_score),
            "case_class": str(body.case_class),
            "ground_truth": int(body.ground_truth),
        }

        with feedback_lock:
            with open(FEEDBACK_PATH, "a") as f:
                f.write(json.dumps(entry) + "\n")

            all_feedback = [json.loads(l) for l in FEEDBACK_PATH.read_text().splitlines() if l.strip()]
            if len(all_feedback) % 10 == 0:
                print(f"Retraining with {len(all_feedback)} feedback entries...")
                new_models = train(feedback=all_feedback)
                state["models"] = new_models

        return {"status": "logged", "feedback_count": len(all_feedback)}

    @app.get("/metrics")
    def metrics_ep():
        if METRICS_PATH.exists():
            return Response(content=METRICS_PATH.read_text(), media_type="application/json")
        return JSONResponse(
            status_code=404,
            content={"error": "metrics not yet computed — POST /feedback or retrain"},
        )

    port = int(os.environ.get("CALIBRATION_PORT", 5001))
    print(f"Calibration sidecar listening on port {port}")
    uvicorn.run(app, host="0.0.0.0", port=port)


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
