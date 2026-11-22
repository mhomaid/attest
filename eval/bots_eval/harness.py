"""Ask the investigator each question through the gateway and score it.

Two phases keep the key apart from the model: `ask` runs the conversation and never sees an
`AnswerKey`; `score` compares the finished transcript to the key afterwards. Before the first
model call, `assert_no_key_in_context` refuses to continue if the system prompt or question text
contains an answer or an evidence id.
"""

from __future__ import annotations

import json
import re
from dataclasses import asdict, dataclass, field
from pathlib import Path

from .gateway import GatewayClient
from .loader import AnswerKey, Question
from .models import SYSTEM_PROMPT

SCHEMA_VERSION = 1
DEFAULT_RESULT_PATH = "eval/results/investigator-bots.json"
MAX_STEPS = 6


class LeakError(AssertionError):
    """The answer key would have reached the model context."""


def _norm(text: str) -> str:
    return re.sub(r"\s+", " ", str(text)).strip().casefold()


def _contains_token(haystack: str, needle: str) -> bool:
    if not needle.strip():
        return False
    # A trailing period ends the token. A period that continues a name (wks-0007.example) does not.
    return (
        re.search(
            rf"(?<![\w-]){re.escape(needle.strip())}(?![\w-]|\.(?=[\w-]))",
            haystack,
            re.IGNORECASE,
        )
        is not None
    )


def assert_no_key_in_context(messages: list[dict], key: AnswerKey) -> None:
    """Check the prompt-side messages (system and user). Tool results are data and may
    legitimately contain the answer; the key itself must never be in what we write."""
    for msg in messages:
        if msg["role"] not in ("system", "user"):
            continue
        for secret in (*key.all_answers(), *key.evidence_ids):
            if _contains_token(msg["content"], secret):
                raise LeakError(f"answer key for {key.question_id} found in a {msg['role']} message")


@dataclass
class Transcript:
    question_id: str
    messages: list[dict]
    final: dict | None
    call_ids: list[str]
    call_rows: dict[str, list[list]]
    tool_errors: int
    stopped: str  # "final" | "max_steps" | "bad_step"


def build_prompt(question: Question, table: str) -> list[dict]:
    return [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": f"Question {question.id}: {question.text}\nTable: {table}"},
    ]


def ask(question: Question, model, client: GatewayClient, table: str, messages: list[dict] | None = None) -> Transcript:
    messages = messages if messages is not None else build_prompt(question, table)
    start = len(client.calls)
    final, stopped = None, "max_steps"
    for _ in range(MAX_STEPS):
        step = model.step(question.id, messages)
        if "final" in step:
            final, stopped = step["final"], "final"
            messages.append({"role": "assistant", "content": json.dumps(step)})
            break
        if "tool" not in step:
            stopped = "bad_step"
            break
        messages.append({"role": "assistant", "content": json.dumps(step)})
        result = client.invoke(step["tool"], step.get("args", {}))
        messages.append({"role": "tool", "content": result.to_message()})
    calls = client.calls[start:]
    return Transcript(
        question_id=question.id,
        messages=messages,
        final=final,
        call_ids=[c.call_id for c in calls],
        call_rows={c.call_id: (c.result or {}).get("rows", []) for c in calls},
        tool_errors=sum(1 for c in calls if c.error),
        stopped=stopped,
    )


@dataclass
class QuestionScore:
    question_id: str
    exact_match: bool
    cited_evidence_correct: bool
    answer: str
    citations: list[str]
    tool_calls: int
    tool_errors: int
    stopped: str


def score(transcript: Transcript, key: AnswerKey) -> QuestionScore:
    final = transcript.final or {}
    answer = str(final.get("answer", ""))
    citations = [str(c) for c in final.get("evidence_citations", [])]
    exact = _norm(answer) in {_norm(a) for a in key.all_answers()}
    valid = bool(citations) and all(c in transcript.call_rows for c in citations)
    cells = {str(cell) for c in citations if c in transcript.call_rows for row in transcript.call_rows[c] for cell in row}
    cited = valid and all(e in cells for e in key.evidence_ids)
    return QuestionScore(
        question_id=transcript.question_id,
        exact_match=exact,
        cited_evidence_correct=cited,
        answer=answer,
        citations=citations,
        tool_calls=len(transcript.call_ids),
        tool_errors=transcript.tool_errors,
        stopped=transcript.stopped,
    )


@dataclass
class RunResult:
    schema_version: int
    model: str
    table: str
    row_count: int
    summary: dict
    questions: list[dict] = field(default_factory=list)
    transcripts: list[Transcript] = field(default_factory=list, repr=False)

    def to_json(self) -> dict:
        data = asdict(self)
        data.pop("transcripts")
        return data


def run_eval(
    questions: list[Question],
    key: dict[str, AnswerKey],
    model,
    client_factory,
    table: str = "bots_events",
    row_count: int = 0,
) -> RunResult:
    """`client_factory()` returns a fresh `GatewayClient` per run."""
    missing = [q.id for q in questions if q.id not in key]
    if missing:
        raise ValueError(f"no answer key for questions: {missing}")
    client = client_factory()
    scores, transcripts = [], []
    for q in questions:
        messages = build_prompt(q, table)
        assert_no_key_in_context(messages, key[q.id])
        transcript = ask(q, model, client, table, messages)
        transcripts.append(transcript)
        scores.append(score(transcript, key[q.id]))
    n = max(1, len(scores))
    summary = {
        "n": len(scores),
        "exact_match": sum(s.exact_match for s in scores) / n,
        "cited_evidence": sum(s.cited_evidence_correct for s in scores) / n,
        "both": sum(s.exact_match and s.cited_evidence_correct for s in scores) / n,
    }
    return RunResult(
        schema_version=SCHEMA_VERSION,
        model=model.name,
        table=table,
        row_count=row_count,
        summary=summary,
        questions=[asdict(s) for s in scores],
        transcripts=transcripts,
    )


def write_result(result: RunResult, path: str | Path = DEFAULT_RESULT_PATH) -> Path:
    out = Path(path)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result.to_json(), indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return out
