"""Load a BOTS-shaped export, the published questions, and the answer key.

Three inputs, three loaders, three types. A `Question` has no answer field, so nothing that
holds one can leak the key by accident. The key (`AnswerKey`) is only read by the scorer.
"""

from __future__ import annotations

import csv
import json
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Protocol

KEY_FIELDS = frozenset({"answer", "answers", "answer_key", "evidence_ids", "accepted_answers"})
_IDENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


@dataclass(frozen=True)
class Question:
    """What the model is allowed to see about a question."""

    id: str
    text: str


@dataclass(frozen=True)
class AnswerKey:
    """Ground truth for one question. Never passed to a model."""

    question_id: str
    answer: str
    evidence_ids: tuple[str, ...]
    accepted_answers: tuple[str, ...] = field(default_factory=tuple)

    def all_answers(self) -> tuple[str, ...]:
        return (self.answer, *self.accepted_answers)


def _read_json_or_csv(path: Path) -> list[dict]:
    if path.suffix.lower() == ".csv":
        with path.open(newline="", encoding="utf-8") as fh:
            return [dict(row) for row in csv.DictReader(fh)]
    data = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(data, dict):
        for key in ("rows", "events", "questions", "key", "items"):
            if key in data:
                data = data[key]
                break
    if not isinstance(data, list):
        raise ValueError(f"{path}: expected a list of rows")
    return data


def load_export(path: str | Path) -> list[dict[str, str]]:
    """Load a BOTS-shaped event export (JSON list or CSV). Every cell becomes a string."""
    rows = _read_json_or_csv(Path(path))
    if not rows:
        raise ValueError("export has no rows")
    columns = list(rows[0].keys())
    for col in columns:
        if not _IDENT.match(col):
            raise ValueError(f"unsafe column name in export: {col!r}")
    leaked = KEY_FIELDS.intersection(c.lower() for c in columns)
    if leaked:
        raise ValueError(f"export has answer-key columns, refusing to load: {sorted(leaked)}")
    return [{c: "" if r.get(c) is None else str(r[c]) for c in columns} for r in rows]


def load_questions(path: str | Path) -> list[Question]:
    """Load questions. Refuses a file that carries answer-key fields."""
    out = []
    for row in _read_json_or_csv(Path(path)):
        bad = KEY_FIELDS.intersection(k.lower() for k in row)
        if bad:
            raise ValueError(
                f"question file has answer-key fields {sorted(bad)}; use split_qa_export"
            )
        out.append(Question(id=str(row["id"]), text=str(row["text"])))
    return out


def _parse_key_row(row: dict) -> AnswerKey:
    evidence = row.get("evidence_ids", [])
    if isinstance(evidence, str):
        evidence = [e.strip() for e in evidence.split("|") if e.strip()]
    accepted = row.get("accepted_answers", [])
    if isinstance(accepted, str):
        accepted = [a.strip() for a in accepted.split("|") if a.strip()]
    return AnswerKey(
        question_id=str(row["id"]),
        answer=str(row["answer"]),
        evidence_ids=tuple(str(e) for e in evidence),
        accepted_answers=tuple(str(a) for a in accepted),
    )


def load_answer_key(path: str | Path) -> dict[str, AnswerKey]:
    return {k.question_id: k for k in map(_parse_key_row, _read_json_or_csv(Path(path)))}


def split_qa_export(rows: list[dict]) -> tuple[list[Question], dict[str, AnswerKey]]:
    """Split a published Q&A export, where question and answer share a row, in two."""
    questions = [Question(id=str(r["id"]), text=str(r["text"])) for r in rows]
    key = {k.question_id: k for k in map(_parse_key_row, rows)}
    return questions, key


class WarmSink(Protocol):
    def create_table(self, table: str, columns: list[str]) -> None: ...
    def insert(self, table: str, rows: list[dict[str, str]]) -> None: ...


def _check_ident(name: str) -> str:
    if not _IDENT.match(name):
        raise ValueError(f"unsafe table name: {name!r}")
    return name


def load_into_warm_tier(rows: list[dict[str, str]], sink: WarmSink, table: str = "bots_events") -> int:
    """Load export rows into a warm-tier table. `sink` decides what the table really is:
    an in-memory table in CI, ClickHouse for a live run. Returns the row count."""
    _check_ident(table)
    columns = list(rows[0].keys())
    sink.create_table(table, columns)
    sink.insert(table, rows)
    return len(rows)


class InMemorySink:
    """A sqlite-backed stand-in for the warm tier. All columns are text."""

    def __init__(self) -> None:
        import sqlite3

        self.conn = sqlite3.connect(":memory:", check_same_thread=False)

    def create_table(self, table: str, columns: list[str]) -> None:
        cols = ", ".join(f"{_check_ident(c)} TEXT" for c in columns)
        self.conn.execute(f"CREATE TABLE {_check_ident(table)} ({cols})")

    def insert(self, table: str, rows: list[dict[str, str]]) -> None:
        cols = list(rows[0].keys())
        marks = ", ".join("?" for _ in cols)
        self.conn.executemany(
            f"INSERT INTO {_check_ident(table)} ({', '.join(cols)}) VALUES ({marks})",
            [tuple(r[c] for c in cols) for r in rows],
        )
        self.conn.commit()


class ClickHouseSink:
    """Load into the real warm tier over the ClickHouse HTTP interface.

    `post(url, body)` is injectable so the request shape is testable without a server.
    """

    def __init__(self, base_url: str, post: Callable[[str, bytes], None] | None = None) -> None:
        self.base_url = base_url.rstrip("/")
        self._post = post or self._default_post

    @staticmethod
    def _default_post(url: str, body: bytes) -> None:
        import urllib.request

        req = urllib.request.Request(url, data=body, method="POST")
        with urllib.request.urlopen(req, timeout=60) as resp:  # noqa: S310 - operator-supplied URL
            resp.read()

    def _query_url(self, sql: str) -> str:
        from urllib.parse import quote

        return f"{self.base_url}/?query={quote(sql)}"

    def create_table(self, table: str, columns: list[str]) -> None:
        cols = ", ".join(f"`{_check_ident(c)}` String" for c in columns)
        sql = f"CREATE TABLE IF NOT EXISTS {_check_ident(table)} ({cols}) ENGINE = MergeTree ORDER BY tuple()"
        self._post(self._query_url(sql), b"")

    def insert(self, table: str, rows: list[dict[str, str]]) -> None:
        body = "\n".join(json.dumps(r, separators=(",", ":")) for r in rows).encode()
        self._post(self._query_url(f"INSERT INTO {_check_ident(table)} FORMAT JSONEachRow"), body)
