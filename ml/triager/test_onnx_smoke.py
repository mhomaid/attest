"""ONNX inference smoke tests — requires `artifacts/model.onnx` from the training pipeline."""
from __future__ import annotations

from pathlib import Path

import numpy as np
import onnxruntime as ort
import pytest

ARTIFACTS = Path(__file__).parent / "artifacts"
MODEL = ARTIFACTS / "model.onnx"


@pytest.fixture(scope="module")
def session() -> ort.InferenceSession:
    if not MODEL.is_file():
        pytest.skip("model.onnx not built (run make train-classifier)")
    return ort.InferenceSession(str(MODEL))


def test_onnx_model_io(session: ort.InferenceSession) -> None:
    inp = session.get_inputs()[0]
    assert inp.name
    assert len(inp.shape) >= 2


def test_onnx_brute_force_is_true_positive(session: ort.InferenceSession) -> None:
    inp = session.get_inputs()[0]
    x_tp = np.array([[0.8, 3002.0, 0.7, 3.5, 2.0, 3.0, 0.5, 0.85]], dtype=np.float32)
    out_tp = session.run(None, {inp.name: x_tp})
    assert int(out_tp[0][0]) == 1, f"Expected label=1 for brute-force, got {out_tp[0][0]}"


def test_onnx_benign_is_negative(session: ort.InferenceSession) -> None:
    inp = session.get_inputs()[0]
    x_bn = np.array([[0.1, 3002.0, 0.0, 0.1, 0.0, 9.0, 0.3, 0.05]], dtype=np.float32)
    out_bn = session.run(None, {inp.name: x_bn})
    assert int(out_bn[0][0]) == 0, f"Expected label=0 for benign, got {out_bn[0][0]}"


def test_novelty_artifacts_shapes() -> None:
    mean_path = ARTIFACTS / "novelty_mean.npy"
    inv_path = ARTIFACTS / "novelty_inv_cov.npy"
    if not mean_path.is_file() or not inv_path.is_file():
        pytest.skip("novelty .npy files missing")
    mean = np.load(ARTIFACTS / "novelty_mean.npy")
    inv_cov = np.load(ARTIFACTS / "novelty_inv_cov.npy")
    assert mean.shape == (8,)
    assert inv_cov.shape == (8, 8)
