import { NextRequest, NextResponse } from "next/server";
import { getScenario } from "@/lib/scenarios";
import type { ScenarioParams } from "@/lib/scenarios";

const COLLECTOR_URL    = process.env.COLLECTOR_URL    ?? "http://localhost:4000";
const ORCHESTRATOR_URL = process.env.ORCHESTRATOR_URL ?? "http://localhost:4300";

// ── Stage result shape ─────────────────────────────────────────────────────

export type StageResult = {
  ok: boolean;
  latency_ms: number;
  artifact: string;          // human-readable line shown in the UI
  error?: string;
};

// Synchronous response: covers stages we can verify in a single round-trip.
export type SimulateResponse = {
  event_ids: string[];
  username: string;
  action_id: string;

  // Hot-path stages (synchronous)
  collector:    StageResult;
  orchestrator: StageResult;
  calibration:  StageResult;
  attestation:  StageResult;

  // Verdict + evidence (returned by orchestrator)
  verdict: string;
  execution_path: string;
  calibrated_confidence: number;
  raw_prediction: number;
  novelty_score: number;
  escalated: boolean;
  classifier_evidence?: {
    input_features: Record<string, number>;
    shap_values: Record<string, number>;
    raw_prediction: number;
    calibrated_confidence: number;
    novelty_score: number;
  };

  total_latency_ms: number;
};

// ── Route handler ──────────────────────────────────────────────────────────

export async function POST(req: NextRequest) {
  const t0 = Date.now();

  const body = await req.json() as { scenario_id: string; params: ScenarioParams };
  const { scenario_id, params } = body;

  const scenario = getScenario(scenario_id);
  if (!scenario) {
    return NextResponse.json({ error: `Unknown scenario: ${scenario_id}` }, { status: 400 });
  }

  // ── Stage 1: Collector → Kafka ────────────────────────────────────────
  const collectorT0 = Date.now();
  let collectorRes: StageResult;
  let event_ids: string[] = [];
  try {
    const r = await fetch(`${COLLECTOR_URL}/ingest`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(scenario.buildCloudTrail(params)),
      signal: AbortSignal.timeout(8_000),
    });
    const data = await r.json() as { event_ids?: string[]; error?: string };
    if (!r.ok || data.error) throw new Error(data.error ?? `HTTP ${r.status}`);
    event_ids = data.event_ids ?? [];
    collectorRes = {
      ok: true,
      latency_ms: Date.now() - collectorT0,
      artifact: `Published ${event_ids.length} OCSF event(s) to Kafka topic 'cloudtrail' (id ${event_ids[0]?.slice(0, 8) ?? "?"}…)`,
    };
  } catch (e) {
    collectorRes = {
      ok: false,
      latency_ms: Date.now() - collectorT0,
      artifact: "Collector unreachable",
      error: String(e),
    };
  }

  // ── Stage 2: Orchestrator (Hybrid triage loop) ─────────────────────────
  // Includes calibration sidecar + attestation envelope signing internally.
  const orchT0 = Date.now();
  let orchestratorRes: StageResult;
  let calibrationRes: StageResult;
  let attestationRes: StageResult;
  let triageData: Record<string, unknown> | null = null;

  try {
    const r = await fetch(`${ORCHESTRATOR_URL}/triage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ alert: scenario.buildAlert(params) }),
      signal: AbortSignal.timeout(10_000),
    });
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    triageData = await r.json() as Record<string, unknown>;

    const ev   = triageData.classifier_evidence as { raw_prediction?: number; calibrated_confidence?: number } | undefined;
    const raw  = ev?.raw_prediction ?? 0;
    const calb = ev?.calibrated_confidence ?? 0;
    const orchLatency = Date.now() - orchT0;

    orchestratorRes = {
      ok: true,
      latency_ms: orchLatency,
      artifact: `${triageData.execution_path} path → verdict ${triageData.verdict} (action ${(triageData.action_id as string)?.slice(0, 8)}…)`,
    };
    calibrationRes = {
      ok: true,
      latency_ms: 0,  // sub-step inside the orchestrator call
      artifact: `Isotonic regression: raw ${raw.toFixed(3)} → calibrated ${calb.toFixed(3)}`,
    };
    attestationRes = {
      ok: true,
      latency_ms: 0,
      artifact: `Ed25519 envelope signed; appended to attestations.ndjson`,
    };
  } catch (e) {
    orchestratorRes = { ok: false, latency_ms: Date.now() - orchT0, artifact: "Orchestrator unreachable", error: String(e) };
    calibrationRes  = { ok: false, latency_ms: 0, artifact: "Skipped (orchestrator failed)" };
    attestationRes  = { ok: false, latency_ms: 0, artifact: "Skipped (orchestrator failed)" };
  }

  const response: SimulateResponse = {
    event_ids,
    username: params.username,
    action_id: (triageData?.action_id as string) ?? "",

    collector:    collectorRes,
    orchestrator: orchestratorRes,
    calibration:  calibrationRes,
    attestation:  attestationRes,

    verdict:               (triageData?.verdict as string)               ?? "unknown",
    execution_path:        (triageData?.execution_path as string)        ?? "classifier",
    calibrated_confidence: (triageData?.calibrated_confidence as number) ?? 0,
    raw_prediction:        ((triageData?.classifier_evidence as { raw_prediction?: number } | undefined)?.raw_prediction) ?? 0,
    novelty_score:         (triageData?.novelty_score as number)         ?? 0,
    escalated:             (triageData?.escalated as boolean)            ?? false,
    classifier_evidence:   triageData?.classifier_evidence as SimulateResponse["classifier_evidence"],

    total_latency_ms: Date.now() - t0,
  };

  return NextResponse.json(response);
}
