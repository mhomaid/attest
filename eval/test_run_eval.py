"""T-035: offline golden-set eval fails on a missed true positive or low agreement."""

from __future__ import annotations

import json
import socket
from pathlib import Path

import pytest

from run_eval import (
    DEFAULT_AGREEMENT_THRESHOLD,
    FEATURES,
    band,
    classify,
    evaluate,
    investigator_prompt,
    load_cases,
    main,
)

ROOT = Path(__file__).resolve().parents[1]
CASES = Path(__file__).resolve().parent / "golden_cases" / "ci_golden.json"


def _case(**overrides):
    case = {
        "case_id": "case-1",
        "detection": "brute_force",
        "label": "true_positive",
        "narrative": "User 1001 at tenant-a failed login from 203.0.113.10.",
        "features": {
            "severity_score": 0.92,
            "threat_intel_hit_count": 4,
            "baseline_deviation": 4.2,
            "entity_reputation_score": 0.7,
        },
        "scripted_investigator": {
            "verdict": "true_positive",
            "rationale": "Failures from a documentation address and no later success.",
        },
    }
    case.update(overrides)
    return case


def _write(tmp_path: Path, cases: list[dict]) -> Path:
    path = tmp_path / "cases.json"
    path.write_text(json.dumps({"version": "1", "cases": cases}), encoding="utf-8")
    return path


# ── scorer bands ─────────────────────────────────────────────────────────────


def test_score_bands_are_closed_at_the_thresholds() -> None:
    assert band(0.72) == "true_positive"
    assert band(0.28) == "false_positive"
    assert band(0.5) == "needs_investigation"
    assert band(0.719) == "needs_investigation"
    assert band(0.281) == "needs_investigation"


def test_classifier_uses_only_features() -> None:
    high = _case()
    flipped = _case(label="false_positive")
    assert classify(high) == classify(flipped)
    assert classify(high)["verdict"] == "true_positive"
    low = _case(
        features={
            "severity_score": 0.0,
            "threat_intel_hit_count": 0,
            "baseline_deviation": 0.0,
            "entity_reputation_score": 0.0,
        }
    )
    assert classify(low)["verdict"] == "false_positive"


def test_classifier_refuses_a_missing_feature() -> None:
    features = {name: 0.0 for name in FEATURES}
    del features["severity_score"]
    with pytest.raises(ValueError, match="severity_score"):
        classify(_case(features=features))


def test_investigator_prompt_does_not_contain_the_label() -> None:
    case = _case()
    prompt = investigator_prompt(case)
    assert case["label"] not in prompt
    assert "rationale" not in prompt
    assert "203.0.113.10" in prompt


# ── gates ────────────────────────────────────────────────────────────────────


def test_shipped_golden_set_passes_with_both_paths() -> None:
    result = evaluate(load_cases(CASES), DEFAULT_AGREEMENT_THRESHOLD)
    summary = result["summary"]
    assert summary["passed"] is True
    assert summary["missed_true_positives"] == 0
    assert summary["agreement"] >= DEFAULT_AGREEMENT_THRESHOLD
    assert summary["n"] >= 4
    paths = {row["final_path"] for row in result["cases"]}
    assert paths == {"classifier", "investigator"}
    assert any(row["label"] == "true_positive" for row in result["cases"])


def test_missed_true_positive_fails_even_when_the_agreement_bar_is_zero() -> None:
    caught = _case(case_id="tp-kept")
    missed = _case(
        case_id="tp-missed",
        features={
            "severity_score": 0.0,
            "threat_intel_hit_count": 0,
            "baseline_deviation": 0.0,
            "entity_reputation_score": 0.0,
        },
        scripted_investigator={"verdict": "true_positive", "rationale": "Script still says attack."},
    )
    result = evaluate([caught, missed], 0.0)
    assert result["summary"]["passed"] is False
    assert result["missed_true_positive_ids"] == ["tp-missed"]
    row = next(item for item in result["cases"] if item["case_id"] == "tp-missed")
    assert row["final_verdict"] == "false_positive"
    assert row["final_path"] == "classifier"


def test_agreement_below_the_threshold_fails_without_a_missed_true_positive(tmp_path) -> None:
    """Classifier stays correct, so no true positive is missed, but the script disagrees."""
    cases = [
        _case(case_id="tp-1"),
        _case(case_id="tp-2", detection="data_exfil"),
        _case(
            case_id="fp-1",
            label="false_positive",
            detection="scheduled_job",
            features={
                "severity_score": 0.05,
                "threat_intel_hit_count": 0,
                "baseline_deviation": 0.1,
                "entity_reputation_score": 0.0,
            },
            scripted_investigator={"verdict": "false_positive", "rationale": "Expected job."},
        ),
    ]
    # Two of three confident cases disagree. Final verdicts still match the labels.
    cases[0]["scripted_investigator"] = {"verdict": "false_positive", "rationale": "Disagree."}
    cases[1]["scripted_investigator"] = {"verdict": "false_positive", "rationale": "Disagree."}
    result = evaluate(cases, DEFAULT_AGREEMENT_THRESHOLD)
    assert result["missed_true_positive_ids"] == []
    assert result["summary"]["agreement"] == pytest.approx(1 / 3)
    assert result["summary"]["passed"] is False


def test_agreement_equal_to_the_threshold_passes() -> None:
    cases = [_case(case_id="tp-1"), _case(case_id="tp-2")]
    cases[1]["scripted_investigator"] = {"verdict": "false_positive", "rationale": "Disagree."}
    result = evaluate(cases, 0.5)
    assert result["summary"]["agreement"] == pytest.approx(0.5)
    assert result["summary"]["passed"] is True
    assert result["missed_true_positive_ids"] == []


def test_needs_investigation_uses_the_script_and_not_the_label() -> None:
    case = _case(
        case_id="ambiguous",
        label="true_positive",
        features={
            "severity_score": 1.0,
            "threat_intel_hit_count": 0,
            "baseline_deviation": 0.0,
            "entity_reputation_score": 0.0,
        },
        scripted_investigator={"verdict": "false_positive", "rationale": "Scheduled change window."},
    )
    assert classify(case)["verdict"] == "needs_investigation"
    result = evaluate([_case(), case], 0.0)
    row = next(item for item in result["cases"] if item["case_id"] == "ambiguous")
    assert row["final_path"] == "investigator"
    assert row["final_verdict"] == "false_positive"
    assert row["missed_true_positive"] is True


def test_run_with_no_comparable_pair_fails_closed() -> None:
    case = _case(
        features={
            "severity_score": 1.0,
            "threat_intel_hit_count": 0,
            "baseline_deviation": 0.0,
            "entity_reputation_score": 0.0,
        }
    )
    result = evaluate([case], 0.0)
    assert result["summary"]["comparable"] == 0
    assert result["summary"]["passed"] is False


def test_unknown_label_and_empty_set_are_refused() -> None:
    with pytest.raises(ValueError, match="label"):
        evaluate([_case(label="benign")], 0.9)
    with pytest.raises(ValueError, match="no cases"):
        evaluate([], 0.9)


def test_runner_makes_no_network_calls(tmp_path, monkeypatch) -> None:
    def boom(*_args, **_kwargs):
        raise AssertionError("network call")

    monkeypatch.setattr(socket, "socket", boom)
    monkeypatch.setattr(socket, "create_connection", boom)
    out = tmp_path / "golden-eval.json"
    assert main(["--cases", str(CASES), "--out", str(out)]) == 0
    assert json.loads(out.read_text())["summary"]["passed"] is True


def test_cli_writes_json_then_exits_nonzero_on_a_missed_true_positive(tmp_path) -> None:
    path = _write(
        tmp_path,
        [
            _case(
                case_id="tp-missed",
                features={
                    "severity_score": 0.0,
                    "threat_intel_hit_count": 0,
                    "baseline_deviation": 0.0,
                    "entity_reputation_score": 0.0,
                },
            )
        ],
    )
    out = tmp_path / "nested" / "golden-eval.json"
    code = main(["--cases", str(path), "--out", str(out), "--agreement-threshold", "0"])
    assert code == 1
    data = json.loads(out.read_text())
    assert data["schema_version"] == 1
    assert data["investigator"] == "scripted"
    assert data["summary"]["passed"] is False
    assert data["missed_true_positive_ids"] == ["tp-missed"]
    assert "answer_key" not in out.read_text()


def test_cli_shipped_set_exits_zero(tmp_path, capsys) -> None:
    out = tmp_path / "golden-eval.json"
    assert main(["--out", str(out)]) == 0
    captured = capsys.readouterr()
    assert "PASS" in captured.out
    assert "FAIL" not in captured.err


def test_ci_workflow_runs_the_eval_and_uploads_the_json() -> None:
    text = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    assert "python run_eval.py --out results/golden-eval.json" in text
    assert "name: golden-eval" in text
    assert "path: eval/results/golden-eval.json" in text
    assert "actions/upload-artifact@" in text
