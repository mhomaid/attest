/// <reference types="bun" />
import { beforeEach, describe, expect, setSystemTime, test } from "bun:test";
import type { KafkaTraceStep } from "@/hooks/use-case-trace-ws";
import type { OcsfEvent } from "@/lib/ocsf-to-alert";
import { useCaseStore, type TriageVerdict } from "@/lib/stores/case-store";

const store = () => useCaseStore.getState();

const event = (id: string): OcsfEvent => ({ event_id: id, severity: "high" });

const verdict = (caseId: string): TriageVerdict => ({
  action_id: "a1",
  case_id: caseId,
  verdict: "benign",
  execution_path: "classifier",
  calibrated_confidence: 0.97,
  novelty_score: 0.1,
  escalated: false,
  latency_ms: 3,
});

const step = (kind: string, ts: number): KafkaTraceStep => ({
  case_id: "c1",
  tenant_id: "t1",
  agent_action_id: "a1",
  agent_id: "triager",
  execution_path: "classifier",
  step_kind: kind,
  summary: kind,
  ts,
});

beforeEach(() => {
  useCaseStore.setState({ cases: {} });
  setSystemTime(new Date("2026-01-01T00:00:00Z"));
});

describe("case store", () => {
  test("re-initialising a case refreshes the event but keeps verdict, trace and disposition", () => {
    store().initCase("c1", event("c1"), null);
    store().setVerdict("c1", verdict("c1"));
    store().appendTraceStep("c1", step("envelope", 1));
    store().setOverride("c1", "false_positive");

    store().initCase("c1", { ...event("c1"), severity: "low" }, { regions_seen_30d: ["us-east-1"] });

    const entry = store().cases.c1;
    expect(entry.event.severity).toBe("low");
    expect(entry.baseline).toEqual({ regions_seen_30d: ["us-east-1"] });
    expect(entry.verdict?.verdict).toBe("benign");
    expect(entry.triageStatus).toBe("complete");
    expect(entry.traceSteps).toHaveLength(1);
    expect(entry.disposition).toBe("override");
    expect(entry.overrideLabel).toBe("false_positive");
  });

  test("triage lifecycle: start → fail → start clears the error → verdict completes", () => {
    store().initCase("c1", event("c1"), null);

    store().startTriage("c1");
    expect(store().cases.c1.triageStatus).toBe("running");
    expect(store().cases.c1.triageStartedAt).toBe(Date.now());

    store().failTriage("c1", "HTTP 502");
    expect(store().cases.c1).toMatchObject({ triageStatus: "failed", triageError: "HTTP 502" });

    store().startTriage("c1");
    expect(store().cases.c1.triageError).toBeUndefined();

    store().setVerdict("c1", verdict("c1"));
    expect(store().cases.c1.triageStatus).toBe("complete");
  });

  test("trace steps are deduplicated and newest-first", () => {
    store().initCase("c1", event("c1"), null);
    store().appendTraceStep("c1", step("classifier_complete", 1));
    store().appendTraceStep("c1", step("envelope", 2));
    store().appendTraceStep("c1", step("envelope", 2));

    expect(store().cases.c1.traceSteps.map((s) => s.step_kind)).toEqual([
      "envelope",
      "classifier_complete",
    ]);

    store().clearTraceSteps("c1");
    expect(store().cases.c1.traceSteps).toEqual([]);
  });

  test("writes to an unknown case are no-ops rather than creating partial entries", () => {
    store().setOverride("missing", "false_positive");
    store().setVerdict("missing", verdict("missing"));
    store().appendTraceStep("missing", step("envelope", 1));

    expect(store().cases).toEqual({});
  });

  test("at capacity, a new case evicts the least recently updated one", () => {
    for (let i = 0; i < 50; i++) {
      setSystemTime(new Date(Date.UTC(2026, 0, 1, 0, 0, i)));
      store().initCase(`c${i}`, event(`c${i}`), null);
    }
    // Touch the oldest so the second-oldest becomes the eviction candidate.
    setSystemTime(new Date(Date.UTC(2026, 0, 1, 0, 1, 0)));
    store().setApproved("c0");

    store().initCase("new", event("new"), null);

    const ids = Object.keys(store().cases);
    expect(ids).toHaveLength(50);
    expect(ids).toContain("new");
    expect(ids).toContain("c0");
    expect(ids).not.toContain("c1");
  });

  test("at capacity, re-opening an existing case evicts nothing", () => {
    for (let i = 0; i < 50; i++) {
      setSystemTime(new Date(Date.UTC(2026, 0, 1, 0, 0, i)));
      store().initCase(`c${i}`, event(`c${i}`), null);
    }

    store().initCase("c25", event("c25"), null);

    expect(Object.keys(store().cases)).toHaveLength(50);
    expect(store().cases.c0).toBeDefined();
  });
});
