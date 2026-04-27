#!/usr/bin/env python3
"""Quick smoke test to verify the ONNX model produces correct predictions."""
import numpy as np
import onnxruntime as ort
from pathlib import Path

ARTIFACTS = Path(__file__).parent / "artifacts"

sess = ort.InferenceSession(str(ARTIFACTS / "model.onnx"))
inp = sess.get_inputs()[0]
print(f"Input: name={inp.name}, shape={inp.shape}, type={inp.type}")

# Known brute-force (TP pattern): should predict label=1
x_tp = np.array([[0.8, 3002.0, 0.7, 3.5, 2.0, 3.0, 0.5, 0.85]], dtype=np.float32)
out_tp = sess.run(None, {inp.name: x_tp})
print(f"Brute-force: label={out_tp[0][0]}, P(TP)={out_tp[1][0][1]:.4f}")
assert out_tp[0][0] == 1, f"Expected label=1 for brute-force, got {out_tp[0][0]}"

# Known benign: should predict label=0
x_bn = np.array([[0.1, 3002.0, 0.0, 0.1, 0.0, 9.0, 0.3, 0.05]], dtype=np.float32)
out_bn = sess.run(None, {inp.name: x_bn})
print(f"Benign:      label={out_bn[0][0]}, P(TP)={out_bn[1][0][1]:.4f}")
assert out_bn[0][0] == 0, f"Expected label=0 for benign, got {out_bn[0][0]}"

# Verify novelty parameters load correctly
mean = np.load(ARTIFACTS / "novelty_mean.npy")
inv_cov = np.load(ARTIFACTS / "novelty_inv_cov.npy")
print(f"Novelty mean shape: {mean.shape}, inv_cov shape: {inv_cov.shape}")
assert mean.shape == (8,), f"Expected mean shape (8,), got {mean.shape}"
assert inv_cov.shape == (8, 8), f"Expected inv_cov shape (8, 8), got {inv_cov.shape}"

print("\nAll smoke tests PASSED")
