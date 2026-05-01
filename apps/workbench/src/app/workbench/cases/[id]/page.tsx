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

  // Fetch the live event and the persisted snapshot in parallel — both are
  // independent queries and each navigation needs both to decide whether to
  // re-upsert. Doing them sequentially used to double the time-to-first-paint.
  const [liveEvent, persisted] = await Promise.all([
    fetchEvent(id),
    getCase(id).catch(() => null),
  ]);

  if (!liveEvent) {
    if (persisted?.event) {
      return (
        <CaseInvestigationClient
          eventId={id}
          ocsfEvent={persisted.event as OcsfEvent}
          liveBaseline={
            persisted.baseline as { regions_seen_30d?: string[] } | null
          }
        />
      );
    }
    return <CaseStoreFallback caseId={id} />;
  }

  // Skip baseline + upsert when the persisted snapshot already matches the
  // live event — most navigations back to a previously-viewed case fall into
  // this path and were doing 2 unnecessary writes/fetches before.
  const persistedEvent = persisted?.event as Record<string, unknown> | undefined;
  const eventUnchanged =
    persistedEvent?.event_id !== undefined &&
    String(persistedEvent.event_id) === String(liveEvent.event_id);

  const liveBaseline = eventUnchanged
    ? (persisted?.baseline as { regions_seen_30d?: string[] } | null) ?? null
    : liveEvent.actor_user_name
      ? await fetchBaseline(liveEvent.actor_user_name)
      : null;

  if (!eventUnchanged) {
    // Fire-and-forget: the snapshot is for fallback rendering only, so a
    // failed upsert must not block the page render or stall the route.
    void upsertCaseSnapshot({
      caseId: id,
      event: liveEvent,
      baseline: liveBaseline,
    }).catch((err) => {
      console.error("[case-page] upsertCaseSnapshot failed", err);
    });
  }

  return (
    <CaseInvestigationClient
      eventId={id}
      ocsfEvent={liveEvent}
      liveBaseline={liveBaseline}
    />
  );
}
