"use client";

import { CaseWorkbench } from "@/components/workbench/case-workbench";
import type { CaseRecord, FeatureImpact } from "@/lib/mock-data";
import type { KafkaTraceStep } from "@/hooks/use-case-trace-ws";

const featureImpacts: FeatureImpact[] = [
  { name: "severity score", value: "0.92", impact: 0.35 },
  { name: "entity reputation", value: "0.41", impact: -0.22 },
  { name: "baseline deviation", value: "2.15", impact: 0.18 },
];

const caseRecord: CaseRecord = {
  id: "phase8-e2e-fixture",
  title: "Phase 8 hybrid triage — fixture",
  severity: "high",
  state: "awaiting-review",
  entity: "fixture-user",
  source: "e2e",
  verdict: "suspicious",
  confidence: 0.86,
  executionPath: "hybrid",
  timeline: [
    { time: "12:00:00", actor: "Fixture", event: "Synthetic case for Playwright." },
  ],
  evidence: [
    { label: "Fixture", value: "non-prod", type: "asset" },
  ],
  featureImpacts,
};

const liveSteps: KafkaTraceStep[] = [
  {
    case_id: "00000000-0000-4000-8000-000000000001",
    tenant_id: "default",
    agent_action_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    agent_id: "investigator-v1",
    execution_path: "hybrid",
    step_kind: "tool_call",
    summary: "query_warm_tier [evidence:11111111-1111-4111-8111-111111111111]",
    ts: Date.now(),
  },
];

/**
 * Static hybrid case for Playwright (no control-plane / orchestrator required).
 * Path is authenticated like the rest of /workbench.
 */
export default function Phase8E2eHybridFixturePage() {
  return (
    <CaseWorkbench
      caseRecord={caseRecord}
      orchestratorCaseId="00000000-0000-4000-8000-000000000001"
      liveSteps={liveSteps}
    />
  );
}
