"""The MCP tool path the investigator uses.

`GatewayClient` speaks the gateway's `POST /invoke` contract. A transport carries the request:
`HttpGatewayTransport` talks to a running `attest-mcp-gateway`; `InProcessGateway` is an
in-memory stand-in for CI that applies the same read-only guards the real policy engine
applies to warm-tier queries (single statement, SELECT only, 10 000-row cap).
"""

from __future__ import annotations

import json
import re
import uuid
from dataclasses import dataclass
from typing import Callable, Protocol

READ_TOOLS = frozenset({"query_hot_tier", "query_warm_tier", "get_user_baseline"})
ROW_CAP = 10_000
_LIMIT = re.compile(r"\blimit\s+(\d+)", re.IGNORECASE)
_FORBIDDEN = re.compile(
    r"\b(insert|update|delete|drop|alter|create|attach|detach|pragma|replace|truncate)\b",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class ToolResult:
    call_id: str
    tool_id: str
    result: dict | None
    error: str | None

    def to_message(self) -> str:
        """What the model reads back. Data and call id only, nothing about scoring."""
        body = {"call_id": self.call_id, "tool_id": self.tool_id}
        if self.error is not None:
            body["error"] = self.error
        else:
            body["result"] = self.result
        return json.dumps(body, separators=(",", ":"))


class Transport(Protocol):
    def invoke(self, request: dict) -> dict: ...


class GatewayClient:
    def __init__(self, transport: Transport, agent_id: str = "eval-investigator", role: str = "investigator"):
        self.transport = transport
        self.agent_id = agent_id
        self.role = role
        self.calls: list[ToolResult] = []

    def invoke(self, tool_id: str, args: dict) -> ToolResult:
        request = {
            "agent_id": self.agent_id,
            "action_id": str(uuid.uuid4()),
            "agent_role": self.role,
            "tool_id": tool_id,
            "args": args,
        }
        response = self.transport.invoke(request)
        call_id = str(response.get("call_log", {}).get("call_id") or uuid.uuid4())
        out = ToolResult(call_id, tool_id, response.get("result"), response.get("error"))
        self.calls.append(out)
        return out


class HttpGatewayTransport:
    """POST /invoke against a running gateway. Used only by a live run."""

    def __init__(self, base_url: str, opener: Callable | None = None):
        self.base_url = base_url.rstrip("/")
        self._opener = opener

    def invoke(self, request: dict) -> dict:
        import urllib.error
        import urllib.request

        req = urllib.request.Request(
            f"{self.base_url}/invoke",
            data=json.dumps(request).encode(),
            headers={"content-type": "application/json"},
            method="POST",
        )
        opener = self._opener or urllib.request.urlopen
        try:
            with opener(req, timeout=60) as resp:
                return json.loads(resp.read())
        except urllib.error.HTTPError as err:
            # The gateway returns the InvokeResponse body on 403/404/500 too.
            return json.loads(err.read())


class InProcessGateway:
    """CI stand-in for the gateway, backed by an `InMemorySink` table."""

    def __init__(self, sink):
        self.sink = sink

    def invoke(self, request: dict) -> dict:
        call_id = str(uuid.uuid4())
        log = {"call_id": call_id, "tool_id": request["tool_id"]}
        denial = self._authorize(request)
        if denial:
            return {"result": None, "call_log": log, "error": f"Deny: {denial}"}
        sql = request["args"].get("sql", "")
        try:
            rows = [list(r) for r in self.sink.conn.execute(sql).fetchall()]
        except Exception as exc:  # sqlite3.Error and friends: surface to the model like a tool error
            return {"result": None, "call_log": log, "error": f"warm query error: {exc}"}
        return {
            "result": {"rows": rows, "row_count": len(rows), "source": "in_memory_warm_tier"},
            "call_log": log,
            "error": None,
        }

    @staticmethod
    def _authorize(request: dict) -> str | None:
        tool, role = request["tool_id"], request["agent_role"]
        if tool not in READ_TOOLS and not (role == "investigator" and tool == "request_human_review"):
            return f"{role} may not invoke {tool}"
        if tool != "query_warm_tier":
            return f"{tool} is not available in the in-memory gateway"
        args = request["args"]
        sql = args.get("sql")
        if not isinstance(sql, str):
            return "query_warm_tier requires string 'sql'"
        if ";" in sql:
            return "stacked SQL statements are not allowed"
        if not re.match(r"\s*(select|with)\b", sql, re.IGNORECASE) or _FORBIDDEN.search(sql):
            return "only read-only SELECT statements are allowed"
        if int(args.get("limit", 0) or 0) > ROW_CAP:
            return f"query limit exceeds {ROW_CAP}-row exfil cap"
        if any(int(n) > ROW_CAP for n in _LIMIT.findall(sql)):
            return f"SQL LIMIT exceeds {ROW_CAP}-row exfil cap"
        return None
