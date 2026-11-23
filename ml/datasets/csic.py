"""Turn synthetic CSIC-shaped HTTP requests into HTTP Activity fixtures.

Callers pass request text they already hold. This module does not fetch a
corpus and does not mark the dataset trainable. `write_fixtures` is the only
function that creates a file, and it refuses a path inside this repository.
"""

from __future__ import annotations

import json
from pathlib import Path
from urllib.parse import unquote, urlsplit

POSITIVE = "anomalous"
LABELS = frozenset({"normal", "anomalous"})
_METHODS = frozenset({"GET", "POST", "PUT", "HEAD", "DELETE"})
_MARKERS = (
    "' or '",
    '" or "',
    " or 1=1",
    "union select",
    "<script",
    "javascript:",
    "../",
    "..\\",
    "\r\n",
    "<!--#",
)


class FixtureDumpError(ValueError):
    """A fixture dump would have landed inside the repository tree."""


def parse_csic_request(raw: str) -> dict:
    """Parse one request in the public CSIC text shape.

    The request line is `METHOD absolute-url HTTP/1.x`, followed by headers
    and an optional body after a blank line.
    """
    text = raw.strip("\n")
    if "\n\n" in text:
        head, body = text.split("\n\n", 1)
    else:
        head, body = text, ""
    lines = [line for line in head.splitlines() if line.strip()]
    if not lines:
        raise ValueError("CSIC-shaped request is empty")
    parts = lines[0].split(" ")
    if len(parts) != 3 or not parts[2].startswith("HTTP/1."):
        raise ValueError("CSIC-shaped request line must be METHOD absolute-url HTTP/1.x")
    method, url, version = parts
    if method not in _METHODS:
        raise ValueError(f"unsupported method {method}")
    if not (url.startswith("http://") or url.startswith("https://")):
        raise ValueError("CSIC-shaped request URL must be absolute")
    headers: dict[str, str] = {}
    for line in lines[1:]:
        if ":" not in line:
            raise ValueError("header line is not CSIC-shaped")
        name, value = line.split(":", 1)
        headers[name.strip().lower()] = value.strip()
    if "host" not in headers:
        raise ValueError("CSIC-shaped request needs a Host header")
    return {
        "method": method,
        "url": url,
        "version": version,
        "headers": headers,
        "body": body.strip("\n"),
    }


def to_http_activity(
    parsed: dict,
    *,
    label: str,
    time: str,
    tenant_id: str = "tenant-a",
) -> dict:
    """Map one parsed request to an OCSF HTTP Activity fixture (class 4002)."""
    if label not in LABELS:
        raise ValueError("label must be normal or anomalous")
    split = urlsplit(parsed["url"])
    return {
        "class_uid": "4002",
        "category_uid": "4",
        "time": time,
        "tenant_id": tenant_id,
        "http_path": split.path or "/",
        "query_string": split.query,
        "http_request": {
            "http_method": parsed["method"],
            "url": parsed["url"],
            "user_agent": parsed["headers"].get("user-agent", ""),
            "version": parsed["version"],
            "body": parsed["body"],
        },
        "src_ip": _src_ip(parsed["headers"]),
        "http_status": None,
        "disposition": None,
        "label": label,
    }


def convert_labelled(samples: list[dict]) -> list[dict]:
    """Convert labelled request text to HTTP Activity fixtures. Writes nothing."""
    fixtures = []
    for index, sample in enumerate(samples):
        parsed = parse_csic_request(str(sample["raw"]))
        time = str(sample.get("time") or f"2024-06-01T00:00:{index:02d}Z")
        tenant = str(sample.get("tenant_id") or "tenant-a")
        fixtures.append(
            to_http_activity(parsed, label=str(sample["label"]), time=time, tenant_id=tenant)
        )
    return fixtures


def write_fixtures(fixtures: list[dict], directory: Path) -> Path:
    """Write `csic_http_activity.json` under `directory`, which must be outside the repo."""
    directory = Path(directory)
    _refuse_repo_dump(directory)
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / "csic_http_activity.json"
    _refuse_repo_dump(path)
    path.write_text(json.dumps(fixtures, indent=2) + "\n")
    return path


def edge_attack(activity: dict) -> bool:
    """Flag SQL injection, XSS, path traversal, CRLF, or SSI in the URL or body.

    The eval label is not read.
    """
    haystack = _payload(activity)
    return any(marker in haystack for marker in _MARKERS)


def score_edge_attacks(fixtures: list[dict]) -> dict[str, float | int]:
    """Precision and recall of `edge_attack` against fixture labels."""
    flags = [edge_attack(row) for row in fixtures]
    labels = [str(row["label"]) for row in fixtures]
    return precision_recall(flags, labels)


def precision_recall(flags: list[bool], labels: list[str]) -> dict[str, float | int]:
    if len(flags) != len(labels):
        raise ValueError("flags and labels must be the same length")
    tp = fp = fn = tn = 0
    for flag, label in zip(flags, labels):
        positive = label == POSITIVE
        if flag and positive:
            tp += 1
        elif flag and not positive:
            fp += 1
        elif positive:
            fn += 1
        else:
            tn += 1
    return {
        "precision": tp / (tp + fp) if tp + fp else 0.0,
        "recall": tp / (tp + fn) if tp + fn else 0.0,
        "tp": tp,
        "fp": fp,
        "fn": fn,
        "tn": tn,
    }


def _src_ip(headers: dict[str, str]) -> str | None:
    forwarded = headers.get("x-forwarded-for", "")
    if not forwarded:
        return None
    return forwarded.split(",")[0].strip() or None


def _payload(activity: dict) -> str:
    request = activity.get("http_request") or {}
    raw = f"{request.get('url') or ''}\n{request.get('body') or ''}"
    decoded = unquote(unquote(raw))
    return f"{raw}\n{decoded}".casefold()


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _refuse_repo_dump(path: Path) -> None:
    repo = _repo_root()
    try:
        path.resolve().relative_to(repo)
    except ValueError:
        return
    raise FixtureDumpError(f"refusing to write CSIC fixtures into the repository ({path})")
