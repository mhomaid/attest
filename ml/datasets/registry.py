"""Load the public dataset registry and refuse non-trainable corpora."""

from __future__ import annotations

from pathlib import Path

import yaml


class DatasetNotTrainableError(ValueError):
    """Raised when a training run names a dataset that is not allowed for train."""


def load_registry(path: Path) -> list[dict]:
    raw = yaml.safe_load(Path(path).read_text())
    if not isinstance(raw, dict) or not isinstance(raw.get("datasets"), list):
        raise ValueError(f"{path} must be a mapping with a datasets list")
    return raw["datasets"]


def index_registry(registry: object) -> dict[str, dict]:
    """Accept a YAML document, a list of entries, or an id-to-entry mapping."""
    if isinstance(registry, dict) and isinstance(registry.get("datasets"), list):
        entries = registry["datasets"]
    elif isinstance(registry, dict) and "id" in registry:
        entries = [registry]
    elif isinstance(registry, dict):
        return {str(key): value for key, value in registry.items()}
    else:
        entries = list(registry)  # type: ignore[arg-type]
    indexed: dict[str, dict] = {}
    for entry in entries:
        if not isinstance(entry, dict) or "id" not in entry:
            raise ValueError("each dataset entry needs an id")
        indexed[str(entry["id"])] = entry
    return indexed


def enforce_trainable(dataset_ids: list[str], by_id: dict[str, dict]) -> None:
    for dataset_id in dataset_ids:
        entry = by_id.get(dataset_id)
        if entry is None:
            raise DatasetNotTrainableError(f"{dataset_id} is not in the dataset registry")
        allowed = list(entry.get("allowed_use") or [])
        if "train" not in allowed:
            raise DatasetNotTrainableError(
                f"{dataset_id} is not allowed for training (allowed_use={allowed})"
            )


def provenance_records(dataset_ids: list[str], by_id: dict[str, dict]) -> list[dict[str, str]]:
    return [
        {"id": dataset_id, "checksum": str(by_id[dataset_id]["checksum"])}
        for dataset_id in dataset_ids
    ]
