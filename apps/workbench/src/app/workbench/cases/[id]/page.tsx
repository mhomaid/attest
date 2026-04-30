import { AttestationTracePanel } from "@/components/workbench/attestation-trace-panel";
import { CaseWorkbench } from "@/components/workbench/case-workbench";
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
  case_id: string; // UUID from orchestrator — workbench-api trace key
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

  const liveEvent = await fetchEvent(id);

  // If the event doesn't exist in the backend, show a not-found state.
  if (!liveEvent) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 p-12 text-center">
        <p className="text-lg font-semibold">Case not found</p>
        <p className="max-w-sm text-sm text-muted-foreground">
          Event <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs">{id}</code> was
          not found in the recent events stream. It may have expired from the hot tier or the
          control-plane may be offline.
        </p>
      </div>
    );
  }

  const liveBaseline = liveEvent.actor_user_name
    ? await fetchBaseline(liveEvent.actor_user_name)
    : null;

  const verdict = await runTriage(liveEvent);

  const featureImpacts: FeatureImpact[] =
    verdict?.classifier_evidence?.shap_values
      ? shapToFeatureImpacts(
          verdict.classifier_evidence.shap_values,
          verdict.classifier_evidence.input_features ?? {},
        )
      : [];

  const now = new Date().toLocaleTimeString("en-US", { hour12: false });

  const timeline: CaseRecord["timeline"] = [
    {
      time: now,
      actor: "Collector",
      event: `Normalized CloudTrail ${liveEvent.api_operation ?? "event"} to OCSF.`,
    },
    ...(liveBaseline
      ? [{ time: now, actor: "RisingWave", event: "User baseline fetched from entity_baselines view." }]
      : []),
    ...(verdict
      ? [
          {
            time: now,
            actor: "Orchestrator",
            event: `${verdict.execution_path} path — ${verdict.verdict} verdict in ${verdict.latency_ms}ms.`,
          },
          {
            time: now,
            actor: "Attestation",
            event: `Ed25519 signed envelope. Action ID: ${verdict.action_id}.`,
          },
        ]
      : [{ time: now, actor: "Orchestrator", event: "Triage unavailable — orchestrator offline." }]),
  ];

  const evidence: CaseRecord["evidence"] = [
    { label: "Event ID", value: liveEvent.event_id, type: "ocsf" },
    { label: "API operation", value: liveEvent.api_operation ?? "—", type: "ocsf" },
    { label: "Service", value: liveEvent.api_service ?? "—", type: "ocsf" },
    { label: "Region", value: liveEvent.cloud_region ?? "—", type: "ocsf" },
    ...(liveBaseline?.regions_seen_30d
      ? [{ label: "Baseline regions (30d)", value: liveBaseline.regions_seen_30d.join(", "), type: "asset" as const }]
      : []),
    ...(verdict
      ? [
          { label: "Triage action ID", value: verdict.action_id, type: "asset" as const },
          { label: "Novelty score", value: verdict.novelty_score.toFixed(3), type: "asset" as const },
        ]
      : []),
  ];

  const caseRecord: CaseRecord = {
    id: liveEvent.event_id,
    title: liveEvent.api_operation
      ? `${liveEvent.api_operation}${liveEvent.api_service ? ` via ${liveEvent.api_service}` : ""}`
      : "Cloud Activity Event",
    severity: (liveEvent.severity ?? "medium") as CaseRecord["severity"],
    state: "awaiting-review",
    entity: liveEvent.actor_user_name ?? "unknown",
    source: "AWS CloudTrail (live)",
    verdict: verdict
      ? ((verdict.verdict.toLowerCase()) as CaseRecord["verdict"])
      : "investigating",
    confidence: verdict?.calibrated_confidence ?? 0,
    executionPath: verdict
      ? ((verdict.execution_path) as CaseRecord["executionPath"])
      : "classifier",
    timeline,
    evidence,
    featureImpacts,
  };

  return (
    <>
      <CaseWorkbench caseRecord={caseRecord} />
      {verdict ? (
        <div className="border-t border-border/80 p-3">
          <AttestationTracePanel caseId={verdict.case_id} />
        </div>
      ) : null}
    </>
  );
}
