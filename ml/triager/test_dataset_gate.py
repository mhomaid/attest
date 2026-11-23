"""License gate on the existing triager training entry point."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
REGISTRY_PATH = REPO / "ml" / "datasets" / "registry.yaml"


def test_train_refuses_eval_only_dataset(tmp_path: Path) -> None:
    import train

    fake = [
        {
            "id": "eval-only",
            "source_url": "https://example.com/eval-only",
            "license": "CC0-1.0",
            "allowed_use": ["eval", "fixtures"],
            "checksum": "sha256:" + "11" * 32,
            "citation": "synthetic fixture",
        }
    ]
    with pytest.raises(train.DatasetNotTrainableError):
        train.main(datasets=["eval-only"], registry=fake, artifacts_dir=tmp_path)

    # The shipped CSIC 2010 entry is fixtures and eval only.
    with pytest.raises(train.DatasetNotTrainableError):
        train.main(
            datasets=["csic-2010"],
            registry_path=REGISTRY_PATH,
            artifacts_dir=tmp_path / "real",
        )


def test_train_records_dataset_ids_and_checksums(tmp_path: Path) -> None:
    import train

    checksum = "sha256:" + "ab" * 32
    registry = [
        {
            "id": "fixture-train",
            "source_url": "https://example.com/fixture-train",
            "license": "CC-BY-4.0",
            "allowed_use": ["train", "eval"],
            "checksum": checksum,
            "citation": "synthetic fixture, not the GUIDE corpus",
        }
    ]
    train.main(
        datasets=["fixture-train"],
        registry=registry,
        artifacts_dir=tmp_path,
    )
    metrics = json.loads((tmp_path / "model_metrics.json").read_text())
    assert metrics["datasets"] == [{"id": "fixture-train", "checksum": checksum}]
