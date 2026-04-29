import { CaseWorkbench } from "@/components/workbench/case-workbench";
import { getCaseById } from "@/lib/mock-data";
import type { CaseRecord, FeatureImpact } from "@/lib/mock-data";
import type { OcsfEvent } from "@/lib/ocsf-to-alert";

const CP_URL           = process.env.CONTROL_PLANE_URL  ?? "http://localhost:8080";
const ORCHESTRATOR_URL = process.env.ORCHESTRATOR_URL   ?? "http://localhost:4300";

async function fetchEvent(id: string): Promise<OcsfEvent | null> {
  try {
    const res = await fetch(`${CP_URL}/v1/events/recent?id=${encodeURIComponent(id)}`, {
      next: { revalidate: 0 },
    });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

async function fetchBaseline(username: string): Promise<{ regions_seen_30d?: string[] } | null> {
  try {
    const res = await fetch(
      `${CP_URL}/v1/baselines/user/${encodeURIComponent(username)}`,
      { next: { revalidate: 30 } },
    );
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

type TriageVerdict = {
  action_id: string;
  case_id: string;
  verdict: string;
  execution_path: string;
  calibrated_confidence: number;
  novelty_score: number;
  escalated: boolean;
  escalation_reason?: string;
  latency_ms: number;
  classifier_evidence?: {
    input_features: Record<string, number>;
    shap_values: Record<string, number>;
    raw_prediction: number;
    calibrated_confidence: number;
    novelty_score: number;
  };
};

async function runTriage(alert: object): Promise<TriageVerdict | null> {
  try {
    const res = await fetch(`${ORCHESTRATOR_URL}/triage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ alert }),
      cache: "no-store",
      signal: AbortSignal.timeout(8_000),
    });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

function shapToFeatureImpacts(
  shapValues: Record<string, number>,
  inputFeatures: Record<string, number>,
): FeatureImpact[] {
  return Object.entries(shapValues)
    .sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]))
    .slice(0, 8)
    .map(([name, impact]) => ({
      name: name.replace(/_/g, " "),
      value: (inputFeatures[name] ?? 0).toFixed(2),
      impact,
    }));
}

export default async function CasePage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  // Base from mock (carries default classifier panel data).
  const base = getCaseById(id);

  // Attempt to enrich from live backend.
  const liveEvent = await fetchEvent(id);
  const liveBaseline = liveEvent?.actor_user_name
    ? await fetchBaseline(liveEvent.actor_user_name)
    : null;

  // Run live triage if we have a real event, otherwise use alert from mock.
  const triagePayload = liveEvent ?? { case_id: id, severity_score: 0.5 };
  const verdict = await runTriage(triagePayload);

  // Build feature impacts from real SHAP values if available.
  const liveFeatureImpacts: FeatureImpact[] | undefined =
    verdict?.classifier_evidence?.shap_values
      ? shapToFeatureImpacts(
          verdict.classifier_evidence.shap_values,
          verdict.classifier_evidence.input_features ?? {},
        )
      : undefined;

  const caseRecord: CaseRecord = {
    ...base,
    // Override fields when we have live data.
    ...(liveEvent && {
      id: liveEvent.event_id,
      title: liveEvent.api_operation
        ? `${liveEvent.api_operation}${liveEvent.api_service ? ` via ${liveEvent.api_service}` : ""}`
        : base.title,
      entity: liveEvent.actor_user_name ?? base.entity,
      source: "AWS CloudTrail (live)",
    }),
    // Override verdict fields from real triage.
    ...(verdict && {
      verdict: (verdict.verdict.toLowerCase() as CaseRecord["verdict"]) ?? base.verdict,
      confidence: verdict.calibrated_confidence,
      executionPath: (verdict.execution_path as CaseRecord["executionPath"]) ?? base.executionPath,
    }),
    // Use live SHAP feature impacts if available, else fall back to mock.
    featureImpacts: liveFeatureImpacts ?? base.featureImpacts,
    // Enrich evidence with live baseline regions if available.
    evidence: [
      ...base.evidence,
      ...(liveBaseline?.regions_seen_30d
        ? [
            {
              label: "Baseline regions (30d)",
              value: liveBaseline.regions_seen_30d.join(", "),
              type: "asset" as const,
            },
          ]
        : []),
      ...(verdict
        ? [
            {
              label: "Triage action ID",
              value: verdict.action_id,
              type: "asset" as const,
            },
            {
              label: "Novelty score",
              value: verdict.novelty_score.toFixed(3),
              type: "asset" as const,
            },
          ]
        : []),
    ],
  };

  return <CaseWorkbench caseRecord={caseRecord} />;
}
