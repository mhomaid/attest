"""BOTS-shaped investigator evaluation harness (T-054).

Loads a Boss-of-the-SOC style export into a warm-tier table, asks an investigator model
the published questions through the MCP gateway tool path, and scores exact-match and
cited-evidence correctness. The answer key is held apart from everything a model sees.
"""

from .gateway import GatewayClient, InProcessGateway, HttpGatewayTransport, ToolResult
from .harness import LeakError, RunResult, run_eval, write_result
from .loader import (
    AnswerKey,
    ClickHouseSink,
    InMemorySink,
    Question,
    load_answer_key,
    load_export,
    load_into_warm_tier,
    load_questions,
    split_qa_export,
)
from .models import LiveChatModel, ScriptedModel

__all__ = [
    "AnswerKey",
    "ClickHouseSink",
    "GatewayClient",
    "HttpGatewayTransport",
    "InMemorySink",
    "InProcessGateway",
    "LeakError",
    "LiveChatModel",
    "Question",
    "RunResult",
    "ScriptedModel",
    "ToolResult",
    "load_answer_key",
    "load_export",
    "load_into_warm_tier",
    "load_questions",
    "run_eval",
    "split_qa_export",
    "write_result",
]
