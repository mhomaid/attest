"use client";

import { useEffect, useRef, useState } from "react";
import { useCaseStore, type TriageVerdict } from "@/lib/stores/case-store";

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

  const [hydrated, setHydrated] = useState(false);
  const [serverChecked, setServerChecked] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const firedRef = useRef(false);

  // Rehydrate the Zustand persist store on first client render.
  useEffect(() => {
    const result = useCaseStore.persist.rehydrate();
    if (result instanceof Promise) {
      void result.then(() => setHydrated(true));
    } else {
      setHydrated(true);
    }
  }, []);

  useEffect(() => {
    if (!hydrated) return;
    let mounted = true;
    setServerChecked(false);

    fetch(`/api/cases/${encodeURIComponent(caseId)}/triage`, { cache: "no-store" })
      .then((r) => r.json() as Promise<TriageStatusResponse>)
      .then((data) => {
        if (!mounted) return;
        if (data.verdict) {
          setVerdict(caseId, data.verdict);
          setLoading(false);
          _triageFired.add(caseId);
          return;
        }
        if (data.status === "failed" && data.error) {
          failTriage(caseId, data.error);
          setError(data.error);
          setLoading(false);
          return;
        }
        setServerChecked(true);
      })
      .catch(() => {
        if (mounted) setServerChecked(true);
      });

    return () => {
      mounted = false;
    };
  }, [caseId, hydrated, setVerdict, failTriage]);

  useEffect(() => {
    if (!hydrated) return;
    if (!serverChecked) return;
    if (!hasCase) return;

    // If the store already has a verdict (from this session or a previous one),
    // do not re-fire triage.
    if (storeVerdict !== null) {
      setLoading(false);
      return;
    }

    // Existing sessions may have terminal trace steps but no cached HTTP
    // verdict because the browser was refreshed before the fetch resolved.
    // Do not start another LLM job in that case.
    if (hasTerminalTrace) {
      setLoading(false);
      return;
    }

    // If another mount/refresh already started this case recently, do not
    // launch a second LLM job. Keep the UI in "analysing" and let the WS trace
    // continue streaming. A stale running marker is retryable after the TTL.
    const runningIsFresh =
      triageStatus === "running" &&
      triageStartedAt > 0 &&
      Date.now() - triageStartedAt < RUNNING_TTL_MS;
    if (runningIsFresh) {
      setLoading(true);
      return;
    }

    if (triageStatus === "running") {
      _triageFired.delete(caseId);
    }

    if (!alert || firedRef.current || _triageFired.has(caseId)) return;
    firedRef.current = true;
    _triageFired.add(caseId);
    startTriage(caseId);
    setLoading(true);

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
        if (!mounted) return;
        setLoading(false);
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : "Triage failed";
        failTriage(caseId, message);
        if (!mounted) return;
        setError(message);
        setLoading(false);
      });

    return () => {
      mounted = false;
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
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
