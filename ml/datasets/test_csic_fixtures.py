"""CSIC-shaped HTTP requests become HTTP Activity fixtures at test time.

The rows below are synthetic. They follow the public request shape (method,
absolute URL, HTTP/1.1, headers, optional body) and are not samples from the
2010 corpus. Generated fixture dumps stay in the pytest temp directory.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from datasets.csic import (
    FixtureDumpError,
    convert_labelled,
    edge_attack,
    parse_csic_request,
    score_edge_attacks,
    write_fixtures,
)
from datasets.registry import load_registry

REPO = Path(__file__).resolve().parents[2]
REGISTRY_PATH = REPO / "ml" / "datasets" / "registry.yaml"


def _request(method: str, target: str, body: str = "") -> str:
    lines = [
        f"{method} {target} HTTP/1.1",
        "User-Agent: AttestFixture/1.0",
        "Host: shop.example",
        "Accept: text/html",
        "Connection: close",
    ]
    if body:
        lines.append("Content-Type: application/x-www-form-urlencoded")
        lines.append(f"Content-Length: {len(body)}")
    raw = "\n".join(lines) + "\n\n"
    if body:
        raw += body + "\n"
    return raw


def _samples() -> list[dict]:
    rows = [
        ("normal", _request("GET", "http://shop.example/catalog?id=3")),
        ("normal", _request("GET", "http://shop.example/search?q=red%20widget")),
        ("normal", _request("GET", "http://shop.example/search?q=monitor")),
        ("normal", _request("POST", "http://shop.example/cart", "id=1001&qty=1&note=gift")),
        ("anomalous", _request("GET", "http://shop.example/search?q=1%27%20OR%20%271%27%3D%271")),
        ("anomalous", _request("GET", "http://shop.example/item?name=%3Cscript%3Ealert(1)%3C/script%3E")),
        ("anomalous", _request("GET", "http://shop.example/file?path=%2e%2e%2fsecret.txt")),
        (
            "anomalous",
            _request("POST", "http://shop.example/comment", "note=hello%0d%0aX-Injected:%201"),
        ),
    ]
    return [
        {"label": label, "raw": raw, "time": f"2024-06-01T00:00:{index:02d}Z"}
        for index, (label, raw) in enumerate(rows)
    ]


def test_csic_registry_stays_fixtures_and_eval_only() -> None:
    entry = next(item for item in load_registry(REGISTRY_PATH) if item["id"] == "csic-2010")
    assert set(entry["allowed_use"]) == {"fixtures", "eval"}
    assert "train" not in entry["allowed_use"]


def test_converter_scores_edge_attacks_on_test_time_fixtures(tmp_path: Path, capsys) -> None:
    samples = _samples()
    fixtures = convert_labelled(samples)
    assert list(tmp_path.iterdir()) == []

    written = write_fixtures(fixtures, tmp_path)
    loaded = json.loads(written.read_text())
    assert written.parent == tmp_path
    assert len(loaded) == 8
    assert all(row["class_uid"] == "4002" for row in loaded)
    assert loaded[0]["http_path"] == "/catalog"
    assert loaded[0]["http_request"]["http_method"] == "GET"
    assert loaded[3]["http_request"]["body"] == "id=1001&qty=1&note=gift"
    assert "shop.example" in loaded[0]["http_request"]["url"]

    metrics = score_edge_attacks(loaded)
    print(
        f"csic edge-attack precision={metrics['precision']:.3f} "
        f"recall={metrics['recall']:.3f}"
    )
    captured = capsys.readouterr()
    assert "precision=1.000" in captured.out
    assert "recall=1.000" in captured.out
    assert metrics == {"precision": 1.0, "recall": 1.0, "tp": 4, "fp": 0, "fn": 0, "tn": 4}

    for row in loaded:
        flipped = dict(row)
        flipped["label"] = "anomalous" if row["label"] == "normal" else "normal"
        assert edge_attack(flipped) is edge_attack(row)


def test_fixture_dumps_are_not_written_into_the_repo(tmp_path: Path) -> None:
    fixtures = convert_labelled(_samples()[:1])
    with pytest.raises(FixtureDumpError):
        write_fixtures(fixtures, REPO / "ml" / "datasets")
    with pytest.raises(FixtureDumpError):
        write_fixtures(fixtures, REPO / "detections")

    written = write_fixtures(fixtures, tmp_path)
    assert written.is_file()
    tracked = subprocess.run(
        ["git", "ls-files", "ml/datasets", "detections"],
        cwd=REPO,
        check=True,
        capture_output=True,
        text=True,
    )
    assert "csic_http_activity.json" not in tracked.stdout
    assert not (REPO / "ml" / "datasets" / "csic_http_activity.json").exists()
    assert not (REPO / "detections" / "csic_http_activity.json").exists()


def test_parser_rejects_a_request_that_is_not_csic_shaped() -> None:
    with pytest.raises(ValueError):
        parse_csic_request("GET /catalog HTTP/1.1\nHost: shop.example\n\n")
    with pytest.raises(ValueError):
        parse_csic_request("not a request\n")
