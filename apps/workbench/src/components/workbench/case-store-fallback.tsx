"use client";

/**
 * Rendered when the server can't find the event in the control-plane hot tier.
 * Attempts to hydrate from the Zustand case store (sessionStorage-persisted).
 *
 * Three outcomes:
 *   1. Store has the case snapshot → render CaseInvestigationClient from cache.
 *   2. Store is empty but rehydration is still pending → show a loading state.
 *   3. Store is empty after rehydration → show the "Event no longer in hot tier" error.
 */

import { useEffect, useState } from "react";
import Link from "next/link";
import { FileX2Icon, LoaderCircleIcon } from "lucide-react";
import { CaseInvestigationClient } from "@/components/workbench/case-investigation-client";
import { useCaseStore, selectCase } from "@/lib/stores/case-store";

export function CaseStoreFallback({ caseId }: { caseId: string }) {
  const [hydrated, setHydrated] = useState(false);
  const cachedEntry = useCaseStore(selectCase(caseId));

  // Rehydrate the persist store on first mount (safe: skipped on server).
  useEffect(() => {
    const result = useCaseStore.persist.rehydrate();
    if (result instanceof Promise) { void result.then(() => setHydrated(true)); }
    else { setHydrated(true); }
  }, []);

  // Still waiting for sessionStorage to load.
  if (!hydrated) {
    return (
      <div className="flex min-h-[320px] items-center justify-center gap-3 text-sm text-muted-foreground">
        <LoaderCircleIcon className="size-5 animate-spin" />
        Loading case from local cache…
      </div>
    );
  }

  // Cache hit — render the full case UI from stored data.
  if (cachedEntry) {
    return (
      <CaseInvestigationClient
        eventId={caseId}
        ocsfEvent={cachedEntry.event}
        liveBaseline={cachedEntry.baseline}
      />
    );
  }

  // Cache miss — show a clean "expired" message.
  return (
    <div className="flex flex-col items-center justify-center gap-4 p-12 text-center">
      <div className="flex h-14 w-14 items-center justify-center rounded-full border border-border bg-muted/30">
        <FileX2Icon className="h-7 w-7 text-muted-foreground" />
      </div>
      <div>
        <p className="text-base font-semibold">Event no longer available</p>
        <p className="mt-1.5 max-w-sm text-sm text-muted-foreground">
          Case{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs">
            {caseId.slice(0, 8)}…
          </code>{" "}
          is not in the hot tier and has no local cache. This happens when the
          pipeline restarts or the session was cleared. Pick a fresh alert from
          the queue.
        </p>
      </div>
      <Link
        href="/workbench/queue"
        className="inline-flex items-center gap-1.5 rounded-md bg-primary px-5 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
      >
        ← Back to Queue
      </Link>
    </div>
  );
}
