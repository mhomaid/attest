"""Feature set v2 for public incident rows.

The column mapping lives in YAML. Code does not name GUIDE fields except by
reading that file. The split is by organization and by time: one organization
never appears in both train and test.
"""

from __future__ import annotations

from pathlib import Path

import pandas as pd
import yaml

DEFAULT_MAP = Path(__file__).resolve().parents[1] / "datasets" / "guide_v2.yaml"


def load_mapping(path: Path | None = None) -> dict:
    raw = yaml.safe_load((path or DEFAULT_MAP).read_text())
    if not isinstance(raw, dict) or "columns" not in raw:
        raise ValueError("feature map must contain a columns list")
    return raw


def row_to_features(row: dict, mapping: dict) -> dict[str, float]:
    out: dict[str, float] = {}
    for spec in mapping["columns"]:
        name = spec["name"]
        source = spec.get("source")
        kind = spec.get("kind", "number")
        value = row.get(source) if source else None
        if kind == "present":
            out[name] = 0.0 if value in (None, "") else 1.0
        elif kind == "hour":
            text = str(value or "")
            hour = 0
            if "T" in text and len(text) >= 13:
                hour = int(text[11:13])
            out[name] = float(hour)
        else:
            try:
                out[name] = float(value) if value not in (None, "") else 0.0
            except (TypeError, ValueError):
                out[name] = 0.0
    return out


def split_by_org_and_time(
    frame: pd.DataFrame,
    org_col: str = "org_id",
    time_col: str = "time",
    train_frac: float = 0.7,
) -> tuple[pd.DataFrame, pd.DataFrame]:
    """Put each organization entirely on one side, ordered by its first event."""
    if not 0.0 < train_frac < 1.0:
        raise ValueError("train_frac must be between 0 and 1")
    first = (
        frame.assign(_t=pd.to_datetime(frame[time_col], utc=True))
        .groupby(org_col, sort=False)["_t"]
        .min()
        .sort_values(kind="mergesort")
    )
    orgs = list(first.index)
    cut = max(1, int(len(orgs) * train_frac))
    if cut >= len(orgs):
        cut = len(orgs) - 1
    train_orgs = set(orgs[:cut])
    test_orgs = set(orgs[cut:])
    if train_orgs & test_orgs:
        raise RuntimeError("organization leaked across the split")
    train = frame[frame[org_col].isin(train_orgs)].copy()
    test = frame[frame[org_col].isin(test_orgs)].copy()
    return train, test
