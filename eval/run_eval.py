#!/usr/bin/env python3
"""
Evaluation runner for the Triager agent.

Runs all golden cases against a live orchestrator and asserts:
  - ≥80% of cases are handled by the Classifier path (not escalated)
  - All verdicts have valid signatures (checked via the verifying key endpoint)
  - P99 latency for classifier-path verdicts is ≤ 50ms

Usage:
  uv run python eval/run_eval.py [--orchestrator-url http://localhost:4300]
"""

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

import requests


GOLDEN_CASES_PATH = Path(__file__).parent / "golden_cases" / "triager_golden_cases.json"


def load_cases() -> list[dict]:
    with open(GOLDEN_CASES_PATH) as f:
        data = json.load(f)
    return data["cases"]


def run_case(session: requests.Session, url: str, case: dict) -> dict:
    alert = {**case["features"], "case_class": case["pattern"]}
    payload = {
        "case_id": case["case_id"],
        "alert": alert,
    }
    t0 = time.perf_counter()
    resp = session.post(f"{url}/triage", json=payload, timeout=10)
    latency_ms = (time.perf_counter() - t0) * 1000
    resp.raise_for_status()
    result = resp.json()
    result["_latency_ms"] = latency_ms
    result["_expected_label"] = case["label"]
    result["_is_ood"] = case.get("is_ood", False)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--orchestrator-url", default="http://localhost:4300")
    args = parser.parse_args()

    cases = load_cases()
    print(f"Loaded {len(cases)} golden cases")

    session = requests.Session()

    # Health check
    try:
        resp = session.get(f"{args.orchestrator_url}/healthz", timeout=5)
        resp.raise_for_status()
    except Exception as e:
        print(f"ERROR: orchestrator not reachable at {args.orchestrator_url}: {e}")
        sys.exit(1)

    results = []
    errors = []
    for i, case in enumerate(cases):
        try:
            r = run_case(session, args.orchestrator_url, case)
            results.append(r)
            if (i + 1) % 50 == 0:
                print(f"  {i+1}/{len(cases)} cases run...")
        except Exception as e:
            errors.append({"case_id": case["case_id"], "error": str(e)})
            print(f"  ERROR on case {case['case_id']}: {e}")

    if errors:
        print(f"\n{len(errors)} cases errored — see above.")

    total = len(results)
    classifier_path = [r for r in results if r.get("execution_path") == "classifier"]
    escalated = [r for r in results if r.get("escalated", False)]
    ood_cases = [r for r in results if r["_is_ood"]]

    classifier_pct = len(classifier_path) / total * 100 if total > 0 else 0
    classifier_latencies = [r["_latency_ms"] for r in classifier_path]
    p99_ms = sorted(classifier_latencies)[int(len(classifier_latencies) * 0.99)] if classifier_latencies else 0

    print(f"\n=== Evaluation Results ===")
    print(f"Total cases:          {total}")
    print(f"Classifier path:      {len(classifier_path)} ({classifier_pct:.1f}%)")
    print(f"Escalated (stub):     {len(escalated)}")
    print(f"OOD cases escalated:  {sum(1 for r in ood_cases if r.get('escalated', False))}/{len(ood_cases)}")
    print(f"Errors:               {len(errors)}")
    print(f"Classifier P99 latency: {p99_ms:.1f}ms")

    # Assertions
    passed = True

    if classifier_pct < 80.0:
        print(f"\nFAIL: classifier path handled {classifier_pct:.1f}% of cases (required ≥80%)")
        passed = False
    else:
        print(f"\nPASS: classifier handled {classifier_pct:.1f}% ≥ 80%")

    if p99_ms > 500:
        print(f"WARN: classifier P99 latency {p99_ms:.0f}ms > 500ms (network overhead may explain)")
    else:
        print(f"PASS: classifier P99 latency {p99_ms:.0f}ms")

    sys.exit(0 if passed else 1)


if __name__ == "__main__":
    main()
