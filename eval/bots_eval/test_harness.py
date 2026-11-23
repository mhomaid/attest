"""T-054 tests: BOTS-shaped harness, scoring, MCP path, and the no-answer-key-in-context guard."""

from __future__ import annotations

import dataclasses
import json
from pathlib import Path

import pytest

from bots_eval import (
    AnswerKey,
    ClickHouseSink,
    GatewayClient,
    InMemorySink,
    InProcessGateway,
    LeakError,
    LiveChatModel,
    Question,
    ScriptedModel,
    load_answer_key,
    load_export,
    load_into_warm_tier,
    load_questions,
    run_eval,
    split_qa_export,
    write_result,
)
from bots_eval.__main__ import main
from bots_eval.harness import assert_no_key_in_context, build_prompt
from bots_eval.models import parse_step

FIX = Path(__file__).parent / "fixtures"


def _setup(export: str = "events.json"):
    rows = load_export(FIX / export)
    sink = InMemorySink()
    load_into_warm_tier(rows, sink)
    gateway = InProcessGateway(sink)
    return rows, sink, gateway


def _run(model, gateway, questions=None, key=None):
    return run_eval(
        questions or load_questions(FIX / "questions.json"),
        key or load_answer_key(FIX / "answer_key.json"),
        model,
        lambda: GatewayClient(gateway),
        row_count=12,
    )


class RecordingModel:
    """Wraps a model and records every message list it is sent."""

    def __init__(self, inner):
        self.inner, self.name, self.seen = inner, inner.name, []

    def step(self, question_id, messages):
        self.seen.append(json.dumps(messages))
        return self.inner.step(question_id, messages)


# ── loading ──────────────────────────────────────────────────────────────────


def test_json_and_csv_exports_load_to_the_same_table() -> None:
    assert load_export(FIX / "events.json") == load_export(FIX / "events.csv")
    assert len(load_export(FIX / "events.csv")) == 12


def test_loader_loads_through_an_in_memory_sink() -> None:
    rows, sink, _ = _setup()
    assert sink.conn.execute("SELECT count(*) FROM bots_events").fetchone()[0] == len(rows)


def test_export_with_answer_columns_is_refused(tmp_path) -> None:
    bad = tmp_path / "bad.json"
    bad.write_text(json.dumps([{"event_id": "e1", "answer": "x"}]))
    with pytest.raises(ValueError, match="answer-key"):
        load_export(bad)


def test_clickhouse_sink_builds_create_and_insert_requests() -> None:
    sent: list[tuple[str, bytes]] = []
    sink = ClickHouseSink("http://clickhouse.example.com:8123/", post=lambda u, b: sent.append((u, b)))
    n = load_into_warm_tier(load_export(FIX / "events.csv"), sink, table="bots_events")
    assert n == 12 and len(sent) == 2
    assert "CREATE%20TABLE%20IF%20NOT%20EXISTS%20bots_events" in sent[0][0]
    assert "FORMAT%20JSONEachRow" in sent[1][0]
    assert len(sent[1][1].decode().splitlines()) == 12


def test_unsafe_table_name_is_refused() -> None:
    with pytest.raises(ValueError, match="unsafe table"):
        load_into_warm_tier(load_export(FIX / "events.json"), InMemorySink(), table="t; DROP TABLE x")


# ── scoring through the MCP path ─────────────────────────────────────────────


def test_scripted_run_scores_exact_match_and_cited_evidence_per_question() -> None:
    _, _, gateway = _setup()
    result = _run(ScriptedModel.from_file(FIX / "scripted_model.json"), gateway)
    by_id = {q["question_id"]: q for q in result.questions}
    assert [by_id[i]["exact_match"] for i in sorted(by_id)] == [True, True, True, False, True]
    assert [by_id[i]["cited_evidence_correct"] for i in sorted(by_id)] == [True, True, True, True, False]
    assert result.summary == {"n": 5, "exact_match": 0.8, "cited_evidence": 0.8, "both": 0.6}
    assert all(q["tool_calls"] == 1 and q["tool_errors"] == 0 for q in result.questions)


def test_fabricated_citation_is_not_correct_evidence() -> None:
    _, _, gateway = _setup()
    script = json.loads((FIX / "scripted_model.json").read_text())
    model = ScriptedModel(script)
    real_step = model.step

    def lying_step(qid, messages):
        out = real_step(qid, messages)
        if "final" in out:
            out["final"]["evidence_citations"].append("not-a-real-call-id")
        return out

    model.step = lying_step
    result = _run(model, gateway)
    assert result.summary["cited_evidence"] == 0.0
    assert result.summary["exact_match"] == 0.8


def test_citing_a_call_that_lacks_the_evidence_is_not_correct() -> None:
    _, _, gateway = _setup()
    script = json.loads((FIX / "scripted_model.json").read_text())
    # Right answer from a query whose rows do not include the key's evidence event.
    script["q-001"]["queries"] = ["SELECT host, host FROM bots_events WHERE event_id = 'evt-0006'"]
    script["q-001"]["answer"] = {"query": 0, "row": 0, "col": 0}
    result = _run(ScriptedModel(script), gateway)
    q1 = result.questions[0]
    assert q1["exact_match"] is False and q1["cited_evidence_correct"] is False


def test_result_json_is_written_for_ci(tmp_path) -> None:
    _, _, gateway = _setup()
    result = _run(ScriptedModel.from_file(FIX / "scripted_model.json"), gateway)
    path = write_result(result, tmp_path / "out" / "investigator-bots.json")
    data = json.loads(path.read_text())
    assert data["schema_version"] == 1 and data["model"] == "scripted"
    assert data["summary"]["n"] == 5 and len(data["questions"]) == 5
    assert "transcripts" not in data
    assert "answer_key" not in path.read_text()


def test_cli_scripted_run_writes_the_ci_artifact(tmp_path, capsys) -> None:
    out = tmp_path / "investigator-bots.json"
    assert main(["--out", str(out)], env={}) == 0
    assert json.loads(out.read_text())["summary"]["exact_match"] == 0.8
    assert "scripted" in capsys.readouterr().out


# ── gateway guards (negative tests) ──────────────────────────────────────────


@pytest.mark.parametrize(
    "sql",
    [
        "SELECT 1; DROP TABLE bots_events",
        "DELETE FROM bots_events",
        "SELECT * FROM bots_events LIMIT 20000",
        "SELECT * FROM bots_events; SELECT 1",
    ],
)
def test_gateway_refuses_non_read_only_or_oversized_queries(sql) -> None:
    _, sink, gateway = _setup()
    result = GatewayClient(gateway).invoke("query_warm_tier", {"sql": sql})
    assert result.error and result.error.startswith("Deny")
    assert sink.conn.execute("SELECT count(*) FROM bots_events").fetchone()[0] == 12


def test_investigator_role_cannot_use_a_response_tool() -> None:
    _, _, gateway = _setup()
    result = GatewayClient(gateway).invoke("edr_isolate_host", {"host": "wks-0007"})
    assert result.error and "may not invoke" in result.error


def test_tool_errors_are_counted_and_the_run_still_scores() -> None:
    _, _, gateway = _setup()
    script = json.loads((FIX / "scripted_model.json").read_text())
    script["q-001"]["queries"] = ["SELECT nope FROM bots_events"]
    script["q-001"]["answer"] = {"query": 0, "row": 0, "col": 0}

    class Tolerant(ScriptedModel):
        def step(self, qid, messages):
            if qid == "q-001" and any(m["role"] == "tool" for m in messages):
                return {"final": {"answer": "", "evidence_citations": []}}
            return super().step(qid, messages)

    result = _run(Tolerant(script), gateway)
    q1 = result.questions[0]
    assert q1["tool_errors"] == 1 and q1["exact_match"] is False


# ── the answer key never reaches the model ───────────────────────────────────


def test_question_type_has_no_answer_fields() -> None:
    names = {f.name for f in dataclasses.fields(Question)}
    assert names == {"id", "text"}


def test_question_file_with_answers_is_refused(tmp_path) -> None:
    mixed = tmp_path / "qa.json"
    mixed.write_text(json.dumps([{"id": "q-1", "text": "Which host?", "answer": "wks-1"}]))
    with pytest.raises(ValueError, match="answer-key"):
        load_questions(mixed)


def test_split_qa_export_separates_question_from_key() -> None:
    questions, key = split_qa_export([{"id": "q-1", "text": "Which host?", "answer": "wks-1", "evidence_ids": "e1|e2"}])
    assert questions == [Question("q-1", "Which host?")]
    assert key["q-1"].answer == "wks-1" and key["q-1"].evidence_ids == ("e1", "e2")


def test_answer_key_is_never_in_the_model_context() -> None:
    """Canary key: none of these strings occur in the table, so any sighting in what the model
    was sent could only have come from the key."""
    _, _, gateway = _setup()
    questions = load_questions(FIX / "questions.json")
    canary = {
        q.id: AnswerKey(q.id, f"CANARY-ANSWER-{q.id}", (f"CANARY-EVID-{q.id}",), (f"CANARY-ALT-{q.id}",))
        for q in questions
    }
    model = RecordingModel(ScriptedModel.from_file(FIX / "scripted_model.json"))
    result = _run(model, gateway, questions, canary)
    assert model.seen, "model was never called"
    blob = "\n".join(model.seen) + json.dumps([t.messages for t in result.transcripts])
    assert "CANARY" not in blob
    assert result.summary["exact_match"] == 0.0  # the key was used for scoring only


def test_real_key_strings_are_absent_from_system_and_user_messages() -> None:
    _, _, gateway = _setup()
    key = load_answer_key(FIX / "answer_key.json")
    questions = load_questions(FIX / "questions.json")
    result = _run(ScriptedModel.from_file(FIX / "scripted_model.json"), gateway)
    for transcript in result.transcripts:
        k = key[transcript.question_id]
        for msg in transcript.messages:
            if msg["role"] in ("system", "user"):
                assert not any(s.lower() in msg["content"].lower() for s in k.evidence_ids)
                assert k.answer.lower() not in msg["content"].lower().replace(transcript.question_id, "")
    assert len(result.transcripts) == len(questions)


def test_harness_refuses_a_prompt_that_contains_the_key() -> None:
    q = Question("q-9", "Which host is wks-0007?")
    key = AnswerKey("q-9", "wks-0007", ("evt-0005",))
    with pytest.raises(LeakError):
        assert_no_key_in_context(build_prompt(q, "bots_events"), key)
    with pytest.raises(LeakError):
        assert_no_key_in_context([{"role": "system", "content": "hint: see evt-0005"}], key)


def test_run_eval_stops_before_calling_a_model_when_the_question_leaks() -> None:
    _, _, gateway = _setup()
    leaky = [Question("q-001", "The answer is wks-0007. Which host connected first?")]
    key = {"q-001": AnswerKey("q-001", "wks-0007", ("evt-0005",))}
    model = RecordingModel(ScriptedModel.from_file(FIX / "scripted_model.json"))
    with pytest.raises(LeakError):
        _run(model, gateway, leaky, key)
    assert model.seen == []


def test_scorer_is_the_only_code_that_reads_the_key() -> None:
    import inspect

    from bots_eval import gateway as gw, models

    for module in (gw, models):
        src = inspect.getsource(module)
        assert "AnswerKey" not in src and "answer_key" not in src


# ── live model is off unless env says so ─────────────────────────────────────


def test_live_model_is_off_by_default() -> None:
    with pytest.raises(RuntimeError, match="off"):
        LiveChatModel.from_env({})
    with pytest.raises(RuntimeError, match="ATTEST_EVAL_LLM_URL"):
        LiveChatModel.from_env({"ATTEST_EVAL_LIVE": "1"})


def test_cli_live_flag_does_not_run_without_the_env_switch(tmp_path) -> None:
    with pytest.raises(RuntimeError, match="off"):
        main(["--live", "--out", str(tmp_path / "x.json")], env={})
    assert list(tmp_path.iterdir()) == []


def test_live_model_sends_only_messages_and_parses_a_tool_step() -> None:
    sent = []

    def post(url, payload):
        sent.append((url, payload))
        return {"choices": [{"message": {"content": 'ok {"tool":"query_warm_tier","args":{"sql":"SELECT 1"}}'}}]}

    model = LiveChatModel("http://llm.example.com/v1", "m", post=post)
    q = Question("q-1", "Which host?")
    step = model.step(q.id, build_prompt(q, "bots_events"))
    assert step == {"tool": "query_warm_tier", "args": {"sql": "SELECT 1"}}
    assert set(sent[0][1]) == {"model", "messages", "temperature"}
    assert parse_step("no json here")["final"]["answer"] == ""
