"""Acceptance checks for the public-data model tasks."""

from __future__ import annotations

import numpy as np
import pandas as pd
import pytest

from public_models import (
    csic_fixtures,
    investigator_prompt,
    login_history_features,
    precision_recall,
    recall_at_fpr,
    require_model_card,
    score_exercise,
    time_split,
    train_guide_v0,
)


def _guide_frame() -> pd.DataFrame:
    rows = []
    for org, start, label in (
        ("org-a", 1, "true_positive"),
        ("org-b", 1, "benign_positive"),
        ("org-c", 1, "false_positive"),
        ("org-d", 2, "true_positive"),
        ("org-e", 3, "benign_positive"),
        ("org-f", 4, "false_positive"),
    ):
        for i in range(8):
            rows.append(
                {
                    "org_id": org,
                    "time": f"2024-06-0{start}T0{i % 9}:00:00Z",
                    "DetectorId": 9 if label == "true_positive" else 1,
                    "Category": "InitialAccess" if label != "false_positive" else "",
                    "MitreTechniques": "T1078" if label == "true_positive" else "",
                    "EntityType": "Ip",
                    "EvidenceCount": 4 if label == "true_positive" else 1,
                    "Timestamp": f"2024-06-0{start}T0{i % 9}:00:00Z",
                    "label": label,
                }
            )
    return pd.DataFrame(rows)


def test_guide_v0_reports_metrics_and_card(tmp_path) -> None:
    metrics = train_guide_v0(_guide_frame(), tmp_path)
    assert 0.0 <= metrics["macro_f1"] <= 1.0
    assert set(metrics["recall"]) == {"false_positive", "benign_positive", "true_positive"}
    assert 0.0 <= metrics["ece"] <= 1.0
    assert (tmp_path / "guide_v0.onnx").is_file()
    assert (tmp_path / "model_card.md").is_file()
    assert "known limits" in (tmp_path / "model_card.md").read_text()
    require_model_card(tmp_path / "guide_v0.onnx")
    bare = tmp_path / "other"
    bare.mkdir()
    (bare / "model.onnx").write_bytes(b"not-a-model")
    with pytest.raises(SystemExit):
        require_model_card(bare / "model.onnx")


def test_login_features_are_streaming_and_time_split() -> None:
    events = pd.DataFrame(
        {
            "user": ["u1", "u1", "u2"],
            "time": [10, 20, 15],
            "device": ["d1", "d2", "d1"],
            "asn": ["1", "1", "2"],
            "country": ["US", "US", "DE"],
            "failed": [1, 0, 0],
            "takeover": [0, 1, 0],
        }
    )
    feats = login_history_features(events)
    second = feats[feats["time"] == 20].iloc[0]
    assert second["new_device"] == 1.0
    assert second["failed_attempts"] == 1.0
    train, test = time_split(feats, train_frac=0.67)
    assert train["time"].max() <= test["time"].min()
    scores = np.array([0.1, 0.9, 0.2])
    labels = np.array([0, 1, 0])
    assert recall_at_fpr(labels, scores, fpr_target=0.5) == 1.0


def test_exercise_keeps_the_answer_out_of_the_prompt() -> None:
    cases = [
        {
            "question": "Which host beaconed?",
            "evidence": "203.0.113.10 received a beacon",
            "evidence_id": "ev-1",
            "answer": "workstation-7",
        }
    ]

    def answer_of(prompt, case):
        assert case["answer"] not in prompt
        return {"answer": "workstation-7", "citations": ["ev-1"]}

    scored = score_exercise(cases, answer_of)
    assert scored["exact_match"] == 1.0
    assert "workstation-7" not in investigator_prompt(cases[0]["question"], cases[0]["evidence"])


def test_csic_fixtures_are_not_written(tmp_path, monkeypatch) -> None:
    fixtures = csic_fixtures(
        [
            {"url": "http://shop.example/search?q=1", "label": "normal"},
            {"url": "http://shop.example/search?q=' OR 1=1", "label": "attack"},
        ]
    )
    assert all(row["class_uid"] == "4002" for row in fixtures)
    scores = precision_recall([False, True], ["normal", "attack"])
    assert scores == {"precision": 1.0, "recall": 1.0}
    assert list(tmp_path.iterdir()) == []
