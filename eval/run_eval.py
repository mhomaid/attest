#!/usr/bin/env python3
"""Offline golden-set eval for CI.

Scores each case with a classifier-style function, then a scripted investigator.
No model file and no network call. The process exits 1 when a labeled true
positive is missed, or when classifier/investigator agreement on confident
cases falls below the threshold.

  uv run python eval/run_eval.py --out eval/results/golden-eval.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

SCHEMA_VERSION = 1
DEFAULT_AGREEMENT_THRESHOLD = 0.90
TP_AT_OR_ABOVE = 0.72
FP_AT_OR_BELOW = 0.28
LABELS = ("true_positive", "false_positive")
FEATURES = (
    "severity_score",
    "threat_intel_hit_count",
    "baseline_deviation",
    "entity_reputation_score",
)

DEFAULT_CASES = Path(__file__).resolve().parent / "golden_cases" / "ci_golden.json"
DEFAULT_OUT = Path(__file__).resolve().parent / "results" / "golden-eval.json"


def load_cases(path: str | Path) -> list[dict]:
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    cases = data["cases"] if isinstance(data, dict) else data
    if not isinstance(cases, list):
        raise ValueError("golden file needs a cases list")
    return cases


def classifier_score(features: dict) -> float:
    """Weighted score in [0, 1]. Each input is a feature, never the label."""
    missing = [name for name in FEATURES if name not in features]
    if missing:
        raise ValueError(f"missing feature: {', '.join(missing)}")
    severity = float(features["severity_score"])
    intel = min(float(features["threat_intel_hit_count"]), 5.0) / 5.0
    deviation = min(float(features["baseline_deviation"]), 5.0) / 5.0
    reputation = float(features["entity_reputation_score"])
    return 0.50 * severity + 0.20 * intel + 0.20 * deviation + 0.10 * reputation


def band(score: float) -> str:
    if score >= TP_AT_OR_ABOVE:
        return "true_positive"
    if score <= FP_AT_OR_BELOW:
        return "false_positive"
    return "needs_investigation"


def classify(case: dict) -> dict:
    score = classifier_score(case["features"])
    return {"verdict": band(score), "score": score}


def investigator_prompt(case: dict) -> str:
    """Text the scripted investigator is allowed to see. The label stays out."""
    features = {name: case["features"][name] for name in FEATURES}
    prompt = (
        f"case_id: {case['case_id']}\n"
        f"detection: {case['detection']}\n"
        f"narrative: {case['narrative']}\n"
        f"features: {json.dumps(features, sort_keys=True)}"
    )
    if str(case.get("label", "")) in prompt:
        raise ValueError(f"label leaked into the investigator prompt for {case.get('case_id', '?')}")
    return prompt


def scripted_verdict(case: dict) -> str:
    investigator_prompt(case)
    try:
        verdict = case["scripted_investigator"]["verdict"]
    except (KeyError, TypeError) as exc:
        raise ValueError(f"{case.get('case_id', '?')} has no scripted investigator verdict") from exc
    if verdict not in LABELS:
        raise ValueError(f"{case.get('case_id', '?')} scripted verdict must be one of {LABELS}")
    return str(verdict)


def evaluate(cases: list[dict], threshold: float) -> dict:
    if not cases:
        raise ValueError("no cases")
    if not 0.0 <= float(threshold) <= 1.0:
        raise ValueError("agreement threshold must be between 0 and 1")

    rows: list[dict] = []
    missed: list[str] = []
    for case in cases:
        case_id = str(case.get("case_id", ""))
        label = case.get("label")
        if label not in LABELS:
            raise ValueError(f"{case_id or '?'} label must be one of {LABELS}")
        for key in ("case_id", "detection", "narrative", "features"):
            if key not in case:
                raise ValueError(f"{case_id or '?'} missing {key}")

        clf = classify(case)
        investigator = scripted_verdict(case)
        if clf["verdict"] == "needs_investigation":
            final = investigator
            path = "investigator"
            counts = False
            agreed = None
        else:
            final = clf["verdict"]
            path = "classifier"
            counts = True
            agreed = clf["verdict"] == investigator
        is_miss = label == "true_positive" and final != "true_positive"
        if is_miss:
            missed.append(case_id)
        rows.append(
            {
                "case_id": case_id,
                "detection": case["detection"],
                "label": label,
                "classifier_verdict": clf["verdict"],
                "classifier_score": round(clf["score"], 6),
                "investigator_verdict": investigator,
                "final_verdict": final,
                "final_path": path,
                "counts_toward_agreement": counts,
                "agreed": agreed,
                "missed_true_positive": is_miss,
            }
        )

    comparable = [row for row in rows if row["counts_toward_agreement"]]
    agreement = (
        sum(1 for row in comparable if row["agreed"]) / len(comparable) if comparable else 0.0
    )
    per_detection: dict[str, dict] = {}
    for row in rows:
        bucket = per_detection.setdefault(
            row["detection"],
            {"n": 0, "comparable": 0, "agreed": 0, "missed_true_positives": 0},
        )
        bucket["n"] += 1
        if row["counts_toward_agreement"]:
            bucket["comparable"] += 1
            if row["agreed"]:
                bucket["agreed"] += 1
        if row["missed_true_positive"]:
            bucket["missed_true_positives"] += 1
    for bucket in per_detection.values():
        bucket["agreement"] = (
            round(bucket["agreed"] / bucket["comparable"], 6) if bucket["comparable"] else None
        )

    passed = not missed and len(comparable) > 0 and agreement >= threshold
    return {
        "schema_version": SCHEMA_VERSION,
        "runner": "golden-set",
        "scorer": "classifier-style",
        "investigator": "scripted",
        "summary": {
            "n": len(rows),
            "comparable": len(comparable),
            "agreement": agreement,
            "agreement_threshold": threshold,
            "missed_true_positives": len(missed),
            "true_positives": sum(1 for row in rows if row["label"] == "true_positive"),
            "passed": passed,
        },
        "missed_true_positive_ids": missed,
        "per_detection": per_detection,
        "cases": rows,
    }


def write_result(result: dict, path: str | Path) -> Path:
    out = Path(path)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return out


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="run_eval")
    parser.add_argument("--cases", type=Path, default=DEFAULT_CASES)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--agreement-threshold", type=float, default=DEFAULT_AGREEMENT_THRESHOLD)
    args = parser.parse_args(argv)

    try:
        result = evaluate(load_cases(args.cases), args.agreement_threshold)
    except ValueError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2

    path = write_result(result, args.out)
    summary = result["summary"]
    print(
        f"golden-set: n={summary['n']} agreement={summary['agreement']:.3f} "
        f"missed_true_positives={summary['missed_true_positives']} "
        f"threshold={summary['agreement_threshold']:.3f} -> {path}"
    )
    if summary["passed"]:
        print("PASS")
        return 0
    for case_id in result["missed_true_positive_ids"]:
        print(f"FAIL: missed true positive {case_id}", file=sys.stderr)
    if summary["agreement"] < summary["agreement_threshold"] or summary["comparable"] == 0:
        print(
            f"FAIL: agreement {summary['agreement']:.3f} < {summary['agreement_threshold']:.3f}",
            file=sys.stderr,
        )
    return 1


if __name__ == "__main__":
    sys.exit(main())
