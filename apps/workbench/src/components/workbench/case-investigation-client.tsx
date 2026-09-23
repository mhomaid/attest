"use client";

import { AttestationTracePanel } from "@/components/workbench/attestation-trace-panel";
import { CaseWorkbench } from "@/components/workbench/case-workbench";
import { analytics } from "@/lib/analytics";
import type { CaseRecord, FeatureImpact } from "@/lib/mock-data";
import type { OcsfEvent } from "@/lib/ocsf-to-alert";
import { useCaseTraceWs } from "@/hooks/use-case-trace-ws";
import { useTriage } from "@/hooks/use-triage";
import { useCaseStore, useCaseStoreHydrated } from "@/lib/stores/case-store";
import { useEffect, useMemo, useRef } from "react";

type ClassifierEvidence = NonNullable<
  ReturnType<typeof useTriage>["verdict"]
>["classifier_evidence"];

function classifierEvidenceToFeatureImpacts(
  evidence: ClassifierEvidence | undefined,
): FeatureImpact[] {
  if (!evidence) return [];

  const shapEntries = Object.entries(evidence.shap_values ?? {});
  if (shapEntries.length > 0) {
    return shapEntries
      .sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]))
      .slice(0, 8)
      .map(([name, impact]) => ({
        name: name.replace(/_/g, " "),
        value: (evidence.input_features?.[name] ?? 0).toFixed(2),
        impact,
      }));
  }

  return Object.entries(evidence.input_features ?? {})
    .sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]))
    .slice(0, 8)
    .map(([name, value]) => ({
      name: name.replace(/_/g, " "),
      value: value.toFixed(2),
      impact: 0,
    }));
}

export function CaseInvestigationClient({
  eventId,
  ocsfEvent,
  liveBaseline,
}: {
  eventId: string;
  ocsfEvent: OcsfEvent;
  liveBaseline: { regions_seen_30d?: string[] } | null;
}) {
  const initCase = useCaseStore((s) => s.initCase);
  const hydrated = useCaseStoreHydrated();

  // Persist the event + baseline once hydrated so the fallback component can
  // render this case even after it expires from the control-plane hot tier.
  useEffect(() => {
    if (hydrated) initCase(eventId, ocsfEvent, liveBaseline);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hydrated, eventId]);

  // Triage fires once on mount — no timeout, orchestrator takes as long as needed.
  const { verdict, loading: triageLoading } = useTriage(eventId, ocsfEvent);

  // WebSocket streams live trace steps from Kafka in real-time.
  const { liveSteps, wsStatus } = useCaseTraceWs(eventId);
  const persistedTraceKeys = useRef<Set<string>>(new Set());

  useEffect(() => {
    analytics.case_opened();
  }, [eventId]);

  useEffect(() => {
    const fresh = liveSteps.filter((s) => {
      const key = `${s.agent_action_id}:${s.step_kind}:${s.ts}`;
      if (persistedTraceKeys.current.has(key)) return false;
      persistedTraceKeys.current.add(key);
      return true;
    });
    if (fresh.length === 0) return;

    void fetch(`/api/cases/${encodeURIComponent(eventId)}/trace`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ steps: fresh }),
    }).catch(() => {
      // The live UI should not fail if persistence is temporarily unavailable.
    });
  }, [eventId, liveSteps]);

  // Derive the full CaseRecord reactively: starts with placeholder values,
  // updates in-place when the triage verdict arrives.
  const caseRecord = useMemo<CaseRecord>(() => {
    const now = new Date().toLocaleTimeString("en-US", { hour12: false });

    const featureImpacts: FeatureImpact[] =
      classifierEvidenceToFeatureImpacts(verdict?.classifier_evidence);

    const timeline: CaseRecord["timeline"] = [
      {
        time: now,
        actor: "Collector",
        event: `Normalized CloudTrail ${ocsfEvent.api_operation ?? "event"} to OCSF.`,
      },
      ...(liveBaseline
        ? [
            {
              time: now,
              actor: "RisingWave",
              event: "User baseline fetched from entity_baselines view.",
            },
          ]
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
        : []),
    ];

    const evidence: CaseRecord["evidence"] = [
      { label: "Event ID", value: ocsfEvent.event_id, type: "ocsf" },
      { label: "API operation", value: ocsfEvent.api_operation ?? "—", type: "ocsf" },
      { label: "Service", value: ocsfEvent.api_service ?? "—", type: "ocsf" },
      { label: "Region", value: ocsfEvent.cloud_region ?? "—", type: "ocsf" },
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
            { label: "Triage action ID", value: verdict.action_id, type: "asset" as const },
            {
              label: "Novelty score",
              value: verdict.novelty_score.toFixed(3),
              type: "asset" as const,
            },
          ]
        : []),
    ];

    return {
      id: ocsfEvent.event_id,
      title: ocsfEvent.api_operation
        ? `${ocsfEvent.api_operation}${ocsfEvent.api_service ? ` via ${ocsfEvent.api_service}` : ""}`
        : "Cloud Activity Event",
      severity: (ocsfEvent.severity ?? "medium") as CaseRecord["severity"],
      state: "awaiting-review",
      entity: ocsfEvent.actor_user_name ?? "unknown",
      source: "AWS CloudTrail (live)",
      verdict: verdict
        ? (verdict.verdict.toLowerCase() as CaseRecord["verdict"])
        : "investigating",
      confidence: verdict?.calibrated_confidence ?? 0,
      executionPath: verdict
        ? (verdict.execution_path as CaseRecord["executionPath"])
        : "classifier",
      timeline,
      evidence,
      featureImpacts,
    };
  }, [ocsfEvent, liveBaseline, verdict]);

  return (
    <>
      <CaseWorkbench
        caseRecord={caseRecord}
        orchestratorCaseId={eventId}
        liveSteps={liveSteps}
        triageLoading={triageLoading}
      />
      <div className="border-t border-border/80 p-3">
        <AttestationTracePanel
          caseId={eventId}
          liveSteps={liveSteps}
          wsStatus={wsStatus}
        />
      </div>
    </>
  );
}
