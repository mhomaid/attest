"""Org-and-time split and the config-driven GUIDE mapping."""

from __future__ import annotations

import pandas as pd

from features_v2 import load_mapping, row_to_features, split_by_org_and_time


def test_split_keeps_each_org_on_one_side() -> None:
    frame = pd.DataFrame(
        {
            "org_id": ["a", "a", "b", "c", "c"],
            "incident_id": ["a1", "a1", "b1", "c1", "c1"],
            "time": [
                "2024-06-01T00:00:00Z",
                "2024-06-01T01:00:00Z",
                "2024-06-02T00:00:00Z",
                "2024-06-03T00:00:00Z",
                "2024-06-03T02:00:00Z",
            ],
        }
    )
    train, test = split_by_org_and_time(frame, train_frac=0.67)
    assert set(train["org_id"]).isdisjoint(set(test["org_id"]))
    assert set(train["incident_id"]).isdisjoint(set(test["incident_id"]))
    # Same rows, same split. Not a random partition.
    again, again_test = split_by_org_and_time(frame, train_frac=0.67)
    assert list(train["org_id"]) == list(again["org_id"])
    assert list(test["org_id"]) == list(again_test["org_id"])


def test_mapping_is_config_not_code() -> None:
    mapping = load_mapping()
    features = row_to_features(
        {
            "DetectorId": 7,
            "Category": "InitialAccess",
            "MitreTechniques": "T1078",
            "EntityType": "Ip",
            "EvidenceCount": 3,
            "Timestamp": "2024-06-04T06:05:15.000Z",
        },
        mapping,
    )
    assert features["detector"] == 7.0
    assert features["category_present"] == 1.0
    assert features["hour_of_day"] == 6.0
    assert features["evidence_count"] == 3.0
