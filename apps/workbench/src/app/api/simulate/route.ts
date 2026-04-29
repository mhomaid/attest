import { NextRequest, NextResponse } from "next/server";
import { getScenario } from "@/lib/scenarios";
import type { ScenarioParams } from "@/lib/scenarios";

const COLLECTOR_URL   = process.env.COLLECTOR_URL    ?? "http://localhost:4000";
const ORCHESTRATOR_URL = process.env.ORCHESTRATOR_URL ?? "http://localhost:4300";

export type SimulateResult = {
  ingest_ok: boolean;
  event_ids: string[];
  ingest_error?: string;
  verdict: string;
  execution_path: string;
  calibrated_confidence: number;
  novelty_score: number;
  escalated: boolean;
  escalation_reason?: string;
  action_id: string;
  latency_ms: number;
  classifier_evidence?: {
    input_features: Record<string, number>;
    shap_values: Record<string, number>;
    calibrated_confidence: number;
    novelty_score: number;
  };
  triage_error?: string;
};

export async function POST(req: NextRequest) {
  const t0 = Date.now();

  const body = await req.json() as { scenario_id: string; params: ScenarioParams };
  const { scenario_id, params } = body;

  const scenario = getScenario(scenario_id);
  if (!scenario) {
    return NextResponse.json({ error: `Unknown scenario: ${scenario_id}` }, { status: 400 });
  }

  const cloudTrailPayload = scenario.buildCloudTrail(params);
  const alertPayload       = scenario.buildAlert(params);

  // Fire both calls concurrently.
  const [ingestResult, triageResult] = await Promise.allSettled([
    fetch(`${COLLECTOR_URL}/ingest`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(cloudTrailPayload),
      signal: AbortSignal.timeout(8_000),
    }).then((r) => r.json()),

    fetch(`${ORCHESTRATOR_URL}/triage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ alert: alertPayload }),
      signal: AbortSignal.timeout(10_000),
    }).then((r) => r.json()),
  ]);

  const ingestOk    = ingestResult.status === "fulfilled";
  const ingestData  = ingestOk ? (ingestResult.value as { event_ids?: string[] }) : null;
  const ingestError = ingestOk ? undefined : String((ingestResult as PromiseRejectedResult).reason);

  const triageOk   = triageResult.status === "fulfilled";
  const triageData = triageOk ? (triageResult.value as Record<string, unknown>) : null;
  const triageError = triageOk ? undefined : String((triageResult as PromiseRejectedResult).reason);

  const result: SimulateResult = {
    ingest_ok: ingestOk,
    event_ids: ingestData?.event_ids ?? [],
    ingest_error: ingestError,

    verdict:               (triageData?.verdict as string)              ?? "unknown",
    execution_path:        (triageData?.execution_path as string)       ?? "classifier",
    calibrated_confidence: (triageData?.calibrated_confidence as number) ?? 0,
    novelty_score:         (triageData?.novelty_score as number)         ?? 0,
    escalated:             (triageData?.escalated as boolean)            ?? false,
    escalation_reason:     triageData?.escalation_reason as string | undefined,
    action_id:             (triageData?.action_id as string)             ?? "",
    latency_ms:            Date.now() - t0,
    classifier_evidence:   triageData?.classifier_evidence as SimulateResult["classifier_evidence"],
    triage_error:          triageError,
  };

  return NextResponse.json(result);
}
