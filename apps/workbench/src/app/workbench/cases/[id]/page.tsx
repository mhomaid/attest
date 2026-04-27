import { CaseWorkbench } from "@/components/workbench/case-workbench";
import { getCaseById } from "@/lib/mock-data";
import type { CaseRecord } from "@/lib/mock-data";
import type { OcsfEvent } from "@/lib/ocsf-to-alert";

const CP_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";

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

export default async function CasePage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  // Base from mock (carries classifier panel data until Phase 4).
  const base = getCaseById(id);

  // Attempt to enrich from live backend.
  const liveEvent = await fetchEvent(id);
  const liveBaseline = liveEvent?.actor_user_name
    ? await fetchBaseline(liveEvent.actor_user_name)
    : null;

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
    ],
  };

  return <CaseWorkbench caseRecord={caseRecord} />;
}
