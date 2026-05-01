import { CaseInvestigationClient } from "@/components/workbench/case-investigation-client";
import { CaseStoreFallback } from "@/components/workbench/case-store-fallback";
import { getCase, upsertCaseSnapshot } from "@/lib/server/case-repository";
import type { OcsfEvent } from "@/lib/ocsf-to-alert";

export const dynamic = "force-dynamic";

const CP_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";

async function fetchEvent(id: string): Promise<OcsfEvent | null> {
  try {
    const res = await fetch(`${CP_URL}/v1/events/recent?id=${encodeURIComponent(id)}`, {
      cache: "no-store",
      signal: AbortSignal.timeout(2000),
    });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

async function fetchBaseline(
  username: string,
): Promise<{ regions_seen_30d?: string[] } | null> {
  try {
    const res = await fetch(
      `${CP_URL}/v1/baselines/user/${encodeURIComponent(username)}`,
      { next: { revalidate: 30 }, signal: AbortSignal.timeout(1500) },
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

  const liveEvent = await fetchEvent(id);

  if (!liveEvent) {
    const persisted = await getCase(id);
    if (persisted?.event) {
      return (
        <CaseInvestigationClient
          eventId={id}
          ocsfEvent={persisted.event as OcsfEvent}
          liveBaseline={persisted.baseline as { regions_seen_30d?: string[] } | null}
        />
      );
    }
    return <CaseStoreFallback caseId={id} />;
  }

  const liveBaseline = liveEvent.actor_user_name
    ? await fetchBaseline(liveEvent.actor_user_name)
    : null;

  await upsertCaseSnapshot({
    caseId: id,
    event: liveEvent,
    baseline: liveBaseline,
  });

  return (
    <CaseInvestigationClient
      eventId={id}
      ocsfEvent={liveEvent}
      liveBaseline={liveBaseline}
    />
  );
}
