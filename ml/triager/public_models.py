"""Train and score the public-data models without downloading a corpus.

GUIDE-shaped rows are mapped through the v2 YAML, split by organization and
time, and fitted in-process. The current `model.onnx` is left in place as the
`demo` model. A new run writes `guide_v0.onnx` and `model_card.md` beside it.
"""

from __future__ import annotations

import hashlib
from pathlib import Path

import numpy as np
import pandas as pd
import onnxmltools
import xgboost as xgb
from onnxmltools.convert.common.data_types import FloatTensorType
from sklearn.metrics import f1_score, recall_score

from features_v2 import load_mapping, row_to_features, split_by_org_and_time

LABELS = ("false_positive", "benign_positive", "true_positive")


def expected_calibration_error(y_true: np.ndarray, proba: np.ndarray, bins: int = 5) -> float:
    pred = proba.argmax(axis=1)
    conf = proba.max(axis=1)
    correct = (pred == y_true).astype(float)
    edges = np.linspace(0.0, 1.0, bins + 1)
    total = len(y_true)
    if total == 0:
        return 0.0
    error = 0.0
    for lo, hi in zip(edges[:-1], edges[1:]):
        mask = (conf >= lo) & (conf <= hi if hi == 1 else conf < hi)
        if not mask.any():
            continue
        error += mask.sum() / total * abs(float(correct[mask].mean()) - float(conf[mask].mean()))
    return float(error)


def guide_matrix(frame: pd.DataFrame, mapping: dict) -> tuple[np.ndarray, np.ndarray, list[str]]:
    names = [spec["name"] for spec in mapping["columns"]]
    rows = []
    labels = []
    for record in frame.to_dict(orient="records"):
        rows.append([row_to_features(record, mapping)[name] for name in names])
        labels.append(LABELS.index(str(record["label"])))
    return np.asarray(rows, dtype=np.float32), np.asarray(labels, dtype=np.int32), names


def train_guide_v0(frame: pd.DataFrame, out_dir: Path, mapping_path: Path | None = None) -> dict:
    """Fit a v0 model. Does not replace artifacts/model.onnx."""
    mapping = load_mapping(mapping_path)
    train, test = split_by_org_and_time(frame)
    x_train, y_train, names = guide_matrix(train, mapping)
    x_test, y_test, _ = guide_matrix(test, mapping)
    model = xgb.XGBClassifier(
        n_estimators=20,
        max_depth=3,
        objective="multi:softprob",
        num_class=3,
        random_state=7,
    )
    model.fit(x_train, y_train)
    proba = model.predict_proba(x_test)
    pred = proba.argmax(axis=1)
    recalls = recall_score(y_test, pred, labels=[0, 1, 2], average=None, zero_division=0)
    metrics = {
        "macro_f1": float(f1_score(y_test, pred, average="macro", zero_division=0)),
        "recall": {LABELS[i]: float(recalls[i]) for i in range(3)},
        "ece": expected_calibration_error(y_test, proba),
        "feature_set": "v2",
        "columns": names,
    }
    out_dir.mkdir(parents=True, exist_ok=True)
    onnx_path = out_dir / "guide_v0.onnx"
    # The production demo model stays at artifacts/model.onnx.
    converted = onnxmltools.convert_xgboost(
        model,
        initial_types=[("float_input", FloatTensorType([None, x_train.shape[1]]))],
    )
    onnxmltools.utils.save_model(converted, str(onnx_path))
    model_hash = hashlib.sha256(onnx_path.read_bytes()).hexdigest()
    col_hash = hashlib.sha256(",".join(names).encode()).hexdigest()
    (out_dir / "model_hash.txt").write_text(model_hash)
    (out_dir / "feature_hash.txt").write_text(col_hash)
    card = out_dir / "model_card.md"
    card.write_text(
        "\n".join(
            [
                "# guide_v0",
                "",
                f"- feature set: v2 ({col_hash})",
                f"- model hash: {model_hash}",
                f"- macro-F1: {metrics['macro_f1']:.4f}",
                f"- expected calibration error: {metrics['ece']:.4f}",
                f"- recall: {metrics['recall']}",
                "- datasets: synthetic GUIDE-shaped rows when the official file has no pinned checksum",
                "- known limits: not trained on the full GUIDE corpus until that file's sha256 is pinned",
                "",
            ]
        )
    )
    metrics["model_hash"] = model_hash
    metrics["column_hash"] = col_hash
    return metrics


def require_model_card(model_path: Path) -> None:
    card = model_path.parent / "model_card.md"
    if not card.is_file():
        raise SystemExit(f"pinned model {model_path} has no model_card.md")


def login_history_features(events: pd.DataFrame) -> pd.DataFrame:
    """Streaming-safe features: each row only sees events at or before its time."""
    ordered = events.sort_values("time", kind="mergesort").copy()
    rows = []
    seen: dict[str, dict] = {}
    for record in ordered.to_dict(orient="records"):
        user = str(record["user"])
        state = seen.setdefault(user, {"devices": set(), "asns": set(), "countries": set(), "last": None, "fails": 0})
        device = str(record["device"])
        asn = str(record["asn"])
        country = str(record["country"])
        last = state["last"]
        gap = 0.0 if last is None else float(record["time"] - last)
        rows.append(
            {
                "user": user,
                "time": record["time"],
                "new_device": 0.0 if device in state["devices"] else 1.0,
                "new_asn": 0.0 if asn in state["asns"] else 1.0,
                "new_country": 0.0 if country in state["countries"] else 1.0,
                "time_since_last": gap,
                "failed_attempts": float(state["fails"]),
                "label": int(record["takeover"]),
            }
        )
        state["devices"].add(device)
        state["asns"].add(asn)
        state["countries"].add(country)
        state["last"] = record["time"]
        if int(record.get("failed", 0)) == 1:
            state["fails"] += 1
    return pd.DataFrame(rows)


def time_split(frame: pd.DataFrame, time_col: str = "time", train_frac: float = 0.7) -> tuple[pd.DataFrame, pd.DataFrame]:
    ordered = frame.sort_values(time_col, kind="mergesort")
    cut = max(1, int(len(ordered) * train_frac))
    if cut >= len(ordered):
        cut = len(ordered) - 1
    return ordered.iloc[:cut].copy(), ordered.iloc[cut:].copy()


def recall_at_fpr(labels: np.ndarray, scores: np.ndarray, fpr_target: float = 0.05) -> float:
    order = np.argsort(-scores)
    labels = labels[order]
    negatives = max(1, int((labels == 0).sum()))
    allowed = fpr_target * negatives
    false_pos = 0
    true_pos = 0
    positives = max(1, int((labels == 1).sum()))
    for label in labels:
        if label == 1:
            true_pos += 1
        else:
            false_pos += 1
            if false_pos > allowed:
                break
    return true_pos / positives


def investigator_prompt(question: str, evidence: str) -> str:
    return f"Question: {question}\nEvidence: {evidence}\n"


def score_exercise(cases: list[dict], answer_of) -> dict:
    """Score a scripted investigator. The answer key is never copied into the prompt."""
    exact = 0
    cited = 0
    for case in cases:
        prompt = investigator_prompt(case["question"], case["evidence"])
        if case["answer"] in prompt:
            raise AssertionError("answer key leaked into the model context")
        reply = answer_of(prompt, case)
        if reply.get("answer") == case["answer"]:
            exact += 1
        if case["evidence_id"] in reply.get("citations", []):
            cited += 1
    n = max(1, len(cases))
    return {"exact_match": exact / n, "cited": cited / n, "n": len(cases)}


def csic_fixtures(requests: list[dict]) -> list[dict]:
    """Turn labelled HTTP requests into HTTP Activity fixtures. Nothing is written."""
    out = []
    for req in requests:
        out.append(
            {
                "class_uid": "4002",
                "http_request": {"url": req["url"], "user_agent": req.get("user_agent", "")},
                "label": req["label"],
            }
        )
    return out


def precision_recall(predicted_attack: list[bool], labels: list[str]) -> dict[str, float]:
    tp = fp = fn = 0
    for pred, label in zip(predicted_attack, labels):
        attack = label == "attack"
        if pred and attack:
            tp += 1
        elif pred and not attack:
            fp += 1
        elif attack and not pred:
            fn += 1
    precision = tp / (tp + fp) if tp + fp else 0.0
    recall = tp / (tp + fn) if tp + fn else 0.0
    return {"precision": precision, "recall": recall}
