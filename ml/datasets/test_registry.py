"""Registry, fetch, gitignore, and data-card checks for public datasets.

Fetch tests use a tiny local fixture. They must not download a real corpus.
"""

from __future__ import annotations

import hashlib
import subprocess
from pathlib import Path

import pytest
import yaml

REPO = Path(__file__).resolve().parents[2]
REGISTRY_PATH = REPO / "ml" / "datasets" / "registry.yaml"
CARDS = REPO / "ml" / "datasets" / "cards"

# RBA and BOTS digests are copied from the publisher. CSIC digests are the
# sha256 of the original RAR archives. GUIDE has no public file digest.
RBA_CHECKSUM = "md5:cc1b1078b3929650e6c08678caffcc57"
BOTS_CHECKSUM = "md5:fd2673726c96e97a39fc03119d6686c6"


def _load_registry() -> list[dict]:
    raw = yaml.safe_load(REGISTRY_PATH.read_text())
    assert isinstance(raw, dict)
    datasets = raw["datasets"]
    assert isinstance(datasets, list)
    return datasets


def _by_id(datasets: list[dict]) -> dict[str, dict]:
    return {entry["id"]: entry for entry in datasets}


def test_registry_lists_public_datasets() -> None:
    by_id = _by_id(_load_registry())
    assert set(by_id) == {"guide", "rba-login", "splunk-bots", "csic-2010"}

    guide = by_id["guide"]
    assert guide["source_url"].startswith("https://")
    assert guide["license"] == "CDLA-Permissive-2.0"
    assert set(guide["allowed_use"]) == {"train", "eval"}
    assert str(guide["checksum"]).startswith(("sha256:", "md5:"))
    assert guide["citation"].strip()

    rba = by_id["rba-login"]
    assert rba["source_url"].startswith("https://")
    assert rba["license"] == "CC-BY-4.0"
    assert set(rba["allowed_use"]) == {"train", "eval"}
    assert rba["checksum"] == RBA_CHECKSUM
    assert rba["citation"].strip()

    bots = by_id["splunk-bots"]
    assert bots["source_url"].startswith("https://")
    assert bots["license"] == "CC0-1.0"
    assert set(bots["allowed_use"]) == {"eval"}
    assert "train" not in bots["allowed_use"]
    assert bots["checksum"] == BOTS_CHECKSUM
    assert bots["citation"].strip()

    csic = by_id["csic-2010"]
    assert csic["source_url"].startswith("http")
    assert set(csic["allowed_use"]) == {"fixtures", "eval"}
    assert "train" not in csic["allowed_use"]
    assert csic["checksum"] == (
        "sha256:8c3bb9091ac3d934d9f9ec0fe576c79171a4ef679acc111d637318e77ea69078"
    )
    files = csic["files"]
    assert [item["filename"] for item in files] == [
        "normalTrafficTraining.rar",
        "normalTrafficTest.rar",
        "anomalousTrafficTest.rar",
    ]
    for item in files:
        algo, digest = item["checksum"].split(":", 1)
        assert algo == "sha256" and len(digest) == 64
    assert csic["citation"].strip()
    # Research-only / no commercial grant must not be marked trainable.
    assert "CC0" not in csic["license"]
    assert "CDLA" not in csic["license"]
    assert "CC-BY" not in csic["license"]


def test_dataset_cache_is_gitignored(tmp_path: Path) -> None:
    cache = REPO / "ml" / "datasets" / "cache"
    cache.mkdir(parents=True, exist_ok=True)
    probe = cache / "probe-not-a-dataset.bin"
    probe.write_bytes(b"synthetic-probe-not-a-corpus\n")
    rel = "ml/datasets/cache/probe-not-a-dataset.bin"
    try:
        ignored = subprocess.run(
            ["git", "check-ignore", "-q", rel],
            cwd=REPO,
            check=False,
        )
        assert ignored.returncode == 0, f"{rel} is not gitignored"
        status = subprocess.run(
            ["git", "status", "--porcelain", "--", "ml/datasets/cache"],
            cwd=REPO,
            check=True,
            capture_output=True,
            text=True,
        )
        assert status.stdout.strip() == ""
    finally:
        probe.unlink(missing_ok=True)


def test_fetch_verifies_checksum_on_fixture(tmp_path: Path) -> None:
    from datasets.fetch import ChecksumMismatch, fetch_dataset

    payload = b"synthetic-fixture-not-a-real-corpus\n"
    digest = hashlib.sha256(payload).hexdigest()
    source = tmp_path / "fixture.bin"
    source.write_bytes(payload)
    registry = tmp_path / "registry.yaml"
    cache = tmp_path / "cache"
    registry.write_text(
        "\n".join(
            [
                "datasets:",
                "  - id: tiny",
                f"    source_url: {source.as_uri()}",
                "    license: CC0-1.0",
                "    allowed_use: [eval]",
                f"    checksum: sha256:{digest}",
                "    citation: synthetic fixture for the fetch gate",
                "",
            ]
        )
    )

    subprocess.run(
        [
            "make",
            "dataset-fetch",
            "ID=tiny",
            f"REGISTRY={registry}",
            f"CACHE={cache}",
        ],
        cwd=REPO,
        check=True,
    )
    stored = cache / "tiny" / "fixture.bin"
    assert stored.read_bytes() == payload

    bad = tmp_path / "registry-bad.yaml"
    bad.write_text(registry.read_text().replace(digest, "0" * 64))
    with pytest.raises(subprocess.CalledProcessError):
        subprocess.run(
            [
                "make",
                "dataset-fetch",
                "ID=tiny",
                f"REGISTRY={bad}",
                f"CACHE={tmp_path / 'cache-bad'}",
            ],
            cwd=REPO,
            check=True,
        )
    assert not any((tmp_path / "cache-bad").rglob("fixture.bin"))
    # Direct call too, so a checksum mismatch is the fetch function's error.
    with pytest.raises(ChecksumMismatch):
        fetch_dataset("tiny", registry_path=bad, cache_dir=tmp_path / "direct-bad")


def test_data_cards_cover_registry() -> None:
    for entry in _load_registry():
        card = CARDS / f"{entry['id']}.md"
        text = card.read_text().lower()
        for heading in ("source", "license", "size", "labels", "known biases"):
            assert heading in text, f"{card.name} missing {heading}"
