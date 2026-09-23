"use client";

import { useEffect, useRef, useState } from "react";
import {
  useCaseStore,
  useCaseStoreHydrated,
  type TriageVerdict,
} from "@/lib/stores/case-store";

// Module-level guard survives React Strict Mode double-mount in dev.
const _triageFired = new Set<string>();
const RUNNING_TTL_MS = 5 * 60 * 1000;

export type { TriageVerdict };

type TriageStatusResponse = {
  status?: "missing" | "idle" | "running" | "complete" | "failed";
  verdict?: TriageVerdict | null;
  error?: string | null;
};

/**
 * Fire-once triage per case.
 *
 * • Reads the verdict from the Zustand store first — if already present
 *   (prior visit in this session, or persisted from a hard refresh), it
 *   returns immediately without hitting the orchestrator again.
 * • Writes the verdict back to the store when it arrives so all consumers
 *   (CaseInvestigationClient, CaseWorkbench, etc.) react in one place.
 */
export function useTriage(caseId: string, alert: object | null) {
  const storeVerdict = useCaseStore((s) => s.cases[caseId]?.verdict ?? null);
  const hasCase = useCaseStore((s) => Boolean(s.cases[caseId]));
  const triageStatus = useCaseStore((s) => s.cases[caseId]?.triageStatus ?? "idle");
  const triageStartedAt = useCaseStore((s) => s.cases[caseId]?.triageStartedAt ?? 0);
  const hasTerminalTrace = useCaseStore((s) =>
    (s.cases[caseId]?.traceSteps ?? []).some((step) =>
      step.step_kind === "final_verdict" || step.step_kind === "envelope",
    ),
  );
  const setVerdict = useCaseStore((s) => s.setVerdict);
  const startTriage = useCaseStore((s) => s.startTriage);
  const failTriage = useCaseStore((s) => s.failTriage);

  const hydrated = useCaseStoreHydrated();
  // Keyed by case so switching cases re-runs the server check without a reset.
  const [serverCheckedFor, setServerCheckedFor] = useState<string | null>(null);
  const serverChecked = serverCheckedFor === caseId;
  const [error, setError] = useState<string | null>(null);
  const firedRef = useRef(false);

  // Existing sessions may have terminal trace steps but no cached HTTP verdict
  // because the browser was refreshed before the fetch resolved; that counts as done.
  const loading = error === null && storeVerdict === null && !hasTerminalTrace;

  useEffect(() => {
    if (!hydrated) return;
    let mounted = true;

    fetch(`/api/cases/${encodeURIComponent(caseId)}/triage`, { cache: "no-store" })
      .then((r) => r.json() as Promise<TriageStatusResponse>)
      .then((data) => {
        if (!mounted) return;
        if (data.verdict) {
          setVerdict(caseId, data.verdict);
          _triageFired.add(caseId);
          return;
        }
        if (data.status === "failed" && data.error) {
          failTriage(caseId, data.error);
          setError(data.error);
          return;
        }
        setServerCheckedFor(caseId);
      })
      .catch(() => {
        if (mounted) setServerCheckedFor(caseId);
      });

    return () => {
      mounted = false;
    };
  }, [caseId, hydrated, setVerdict, failTriage]);

  useEffect(() => {
    if (!hydrated || !serverChecked || !hasCase) return;
    if (storeVerdict !== null || hasTerminalTrace) return;

    // If another mount/refresh already started this case recently, do not
    // launch a second LLM job. Keep the UI in "analysing" and let the WS trace
    // continue streaming. A stale running marker is retryable after the TTL.
    const runningIsFresh =
      triageStatus === "running" &&
      triageStartedAt > 0 &&
      Date.now() - triageStartedAt < RUNNING_TTL_MS;
    if (runningIsFresh) return;

    if (triageStatus === "running") {
      _triageFired.delete(caseId);
    }

    if (!alert || firedRef.current || _triageFired.has(caseId)) return;
    firedRef.current = true;
    _triageFired.add(caseId);
    startTriage(caseId);

    let mounted = true;

    fetch(`/api/cases/${encodeURIComponent(caseId)}/triage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ alert }),
    })
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.json() as Promise<TriageVerdict>;
      })
      .then((data) => {
        // Always write to store (even if navigated away) so the next mount
        // picks it up instantly.
        setVerdict(caseId, data);
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : "Triage failed";
        failTriage(caseId, message);
        if (mounted) setError(message);
      });

    return () => {
      mounted = false;
    };
  }, [
    alert,
    caseId,
    hasCase,
    hydrated,
    serverChecked,
    storeVerdict,
    triageStatus,
    triageStartedAt,
    hasTerminalTrace,
    startTriage,
    setVerdict,
    failTriage,
  ]);

  return { verdict: storeVerdict, loading, error };
}
