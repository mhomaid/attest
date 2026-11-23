"""Investigator models for the harness.

A model is anything with `name` and `step(question_id, messages) -> dict`. A step is either
`{"tool": "<tool_id>", "args": {...}}` or `{"final": {"answer": str, "evidence_citations": [...]}}`.
`messages` is exactly what a real model would be sent: a system prompt, the question, and tool
results. The answer key is not reachable from here.
"""

from __future__ import annotations

import json
import os
import re
from pathlib import Path


class ScriptedModel:
    """Deterministic model for CI. The script says which SQL to run and where in the tool
    result the answer sits; the answer is read from tool results, never from a key.

    Script per question id:
      {"queries": ["SELECT ..."], "answer": {"query": -1, "row": 0, "col": 1} | {"row_count": true},
       "cite": "all" | "none"}
    """

    name = "scripted"

    def __init__(self, script: dict[str, dict]):
        self.script = script

    @classmethod
    def from_file(cls, path: str | Path) -> "ScriptedModel":
        return cls(json.loads(Path(path).read_text(encoding="utf-8")))

    def step(self, question_id: str, messages: list[dict]) -> dict:
        plan = self.script[question_id]
        results = [json.loads(m["content"]) for m in messages if m["role"] == "tool"]
        if len(results) < len(plan["queries"]):
            return {"tool": "query_warm_tier", "args": {"sql": plan["queries"][len(results)]}}
        spec = plan["answer"]
        if spec.get("row_count"):
            answer = str(results[-1]["result"]["row_count"])
        else:
            rows = results[spec["query"]]["result"]["rows"]
            answer = str(rows[spec["row"]][spec["col"]])
        cites = [r["call_id"] for r in results] if plan.get("cite", "all") == "all" else []
        return {"final": {"answer": answer, "evidence_citations": cites}}


SYSTEM_PROMPT = (
    "You are the Investigator. Answer the question using only data returned by tools. "
    "Call query_warm_tier with one read-only SELECT against the table named in the question. "
    "Reply with ONLY one JSON object per turn, either "
    '{"tool":"query_warm_tier","args":{"sql":"..."}} or '
    '{"final":{"answer":"<short answer>","evidence_citations":["<call_id>"]}}. '
    "Cite the call_id of every tool call whose rows support the answer."
)


class LiveChatModel:
    """OpenAI-compatible chat endpoint (for example llama.cpp). Off unless the operator sets
    `ATTEST_EVAL_LIVE=1` and the endpoint variables; nothing is sent anywhere otherwise."""

    def __init__(self, url: str, model: str, api_key: str | None = None, post=None):
        self.url, self.model, self.api_key = url.rstrip("/"), model, api_key
        self.name = f"live:{model}"
        self._post = post or self._default_post

    @classmethod
    def from_env(cls, env=os.environ) -> "LiveChatModel":
        if env.get("ATTEST_EVAL_LIVE") != "1":
            raise RuntimeError("live eval is off; set ATTEST_EVAL_LIVE=1 to enable it")
        missing = [k for k in ("ATTEST_EVAL_LLM_URL", "ATTEST_EVAL_LLM_MODEL") if not env.get(k)]
        if missing:
            raise RuntimeError(f"live eval needs {', '.join(missing)}")
        return cls(env["ATTEST_EVAL_LLM_URL"], env["ATTEST_EVAL_LLM_MODEL"], env.get("ATTEST_EVAL_LLM_API_KEY"))

    def _default_post(self, url: str, payload: dict) -> dict:
        import urllib.request

        headers = {"content-type": "application/json"}
        if self.api_key:
            headers["authorization"] = f"Bearer {self.api_key}"
        req = urllib.request.Request(url, data=json.dumps(payload).encode(), headers=headers, method="POST")
        with urllib.request.urlopen(req, timeout=120) as resp:  # noqa: S310 - operator-supplied URL
            return json.loads(resp.read())

    def step(self, question_id: str, messages: list[dict]) -> dict:
        chat = [{"role": "user" if m["role"] == "tool" else m["role"], "content": m["content"]} for m in messages]
        reply = self._post(
            f"{self.url}/chat/completions",
            {"model": self.model, "messages": chat, "temperature": 0},
        )
        return parse_step(reply["choices"][0]["message"]["content"])


def parse_step(text: str) -> dict:
    match = re.search(r"\{.*\}", text, re.DOTALL)
    if not match:
        return {"final": {"answer": "", "evidence_citations": []}}
    try:
        obj = json.loads(match.group(0))
    except json.JSONDecodeError:
        return {"final": {"answer": "", "evidence_citations": []}}
    if isinstance(obj, dict) and ("tool" in obj or "final" in obj):
        return obj
    return {"final": {"answer": "", "evidence_citations": []}}
