"""CLI. `python -m bots_eval` (scripted model, in-memory table) or `--live` (operator opt-in).

Run from the `eval/` directory, or use `make eval-investigator-ci` / `make eval-investigator`.
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from .gateway import GatewayClient, HttpGatewayTransport, InProcessGateway
from .harness import DEFAULT_RESULT_PATH, run_eval, write_result
from .loader import (
    ClickHouseSink,
    InMemorySink,
    load_answer_key,
    load_export,
    load_into_warm_tier,
    load_questions,
)
from .models import LiveChatModel, ScriptedModel

FIXTURES = Path(__file__).parent / "fixtures"


def main(argv: list[str] | None = None, env=os.environ) -> int:
    p = argparse.ArgumentParser(prog="bots_eval")
    p.add_argument("--live", action="store_true", help="live model + gateway; needs ATTEST_EVAL_LIVE=1")
    p.add_argument("--export", default=env.get("ATTEST_EVAL_EXPORT", str(FIXTURES / "events.json")))
    p.add_argument("--questions", default=env.get("ATTEST_EVAL_QUESTIONS", str(FIXTURES / "questions.json")))
    p.add_argument("--key", default=env.get("ATTEST_EVAL_KEY", str(FIXTURES / "answer_key.json")))
    p.add_argument("--table", default=env.get("ATTEST_EVAL_TABLE", "bots_events"))
    p.add_argument("--out", default=env.get("ATTEST_EVAL_OUT", DEFAULT_RESULT_PATH))
    args = p.parse_args(argv)

    rows = load_export(args.export)
    questions = load_questions(args.questions)
    key = load_answer_key(args.key)

    if args.live:
        model = LiveChatModel.from_env(env)  # raises unless ATTEST_EVAL_LIVE=1
        for var in ("ATTEST_EVAL_CLICKHOUSE_URL", "ATTEST_EVAL_MCP_URL"):
            if not env.get(var):
                print(f"live eval needs {var}", file=sys.stderr)
                return 2
        load_into_warm_tier(rows, ClickHouseSink(env["ATTEST_EVAL_CLICKHOUSE_URL"]), args.table)
        transport = HttpGatewayTransport(env["ATTEST_EVAL_MCP_URL"])
    else:
        model = ScriptedModel.from_file(FIXTURES / "scripted_model.json")
        sink = InMemorySink()
        load_into_warm_tier(rows, sink, args.table)
        transport = InProcessGateway(sink)

    result = run_eval(questions, key, model, lambda: GatewayClient(transport), args.table, len(rows))
    path = write_result(result, args.out)
    s = result.summary
    print(f"{result.model}: exact={s['exact_match']:.2f} cited={s['cited_evidence']:.2f} n={s['n']} -> {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
