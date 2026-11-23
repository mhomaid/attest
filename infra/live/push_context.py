#!/usr/bin/env python3
"""Post one synthetic edge event and one synthetic SIEM alert to the collector.

This is the live sender for local Graylog-shaped alerts and edge firewall
events. It retries until the collector accepts the HMAC, then exits.
No credentials or hostnames from a real deployment.
"""

from __future__ import annotations

import hashlib
import hmac
import json
import os
import time
import urllib.error
import urllib.request

SECRET = os.environ.get("ATTEST_INGEST_HMAC_SECRET", "")
EDGE = os.environ.get("COLLECTOR_EDGE_URL", "http://collector:4000/ingest/edge")
SIEM = os.environ.get("COLLECTOR_SIEM_URL", "http://collector:4000/ingest/graylog")


def sign(body: bytes) -> dict[str, str]:
    stamp = str(int(time.time()))
    mac = hmac.new(SECRET.encode(), digestmod=hashlib.sha256)
    mac.update(stamp.encode())
    mac.update(b".")
    mac.update(body)
    return {
        "Content-Type": "application/json",
        "X-Attest-Timestamp": stamp,
        "X-Attest-Signature": mac.hexdigest(),
        "X-Tenant-Id": "tenant-a",
    }


def post(url: str, payload: dict) -> int:
    body = json.dumps(payload).encode()
    req = urllib.request.Request(url, data=body, headers=sign(body), method="POST")
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            return resp.status
    except urllib.error.HTTPError as exc:
        return exc.code


EDGE_EVENT = {
    "Action": "block",
    "ClientIP": "203.0.113.10",
    "ClientRequestURI": "/hooks/callback",
    "EdgeResponseStatus": 403,
    "RayID": "ray-1001",
    "Datetime": "2024-01-15T10:30:00Z",
}

SIEM_EVENT = {
    "event_definition_id": "def-1001",
    "event_definition_title": "Repeated login failures",
    "event": {
        "id": "evt-1001",
        "timestamp": "2024-01-15T10:30:00.000Z",
        "message": "Repeated login failures",
        "source": "siem.example.com",
        "priority": 2,
        "alert": True,
        "fields": {"user": "shopper-1001"},
    },
    "backlog": [{"message": "failed login", "source": "app.example.com"}],
}


def main() -> None:
    if not SECRET:
        raise SystemExit("ATTEST_INGEST_HMAC_SECRET is empty")
    for _ in range(30):
        edge = post(EDGE, EDGE_EVENT)
        siem = post(SIEM, SIEM_EVENT)
        print(f"edge={edge} siem={siem}", flush=True)
        if edge == 200 and siem == 200:
            return
        time.sleep(2)
    raise SystemExit("collector did not accept the live edge and SIEM posts")


if __name__ == "__main__":
    main()
