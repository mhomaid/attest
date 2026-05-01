"use client";

import { FileWarning, Loader2, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";

type TraceStep = {
  kind: string;
  agent_action_id: string;
  agent_id: string;
  execution_path: string;
  verdict: string;
  tool_call_count: number;
  belief_count: number;
};

type TraceResponse = { steps?: TraceStep[]; error?: string };

async function fetchCaseTrace(caseId: string): Promise<TraceResponse> {
  const res = await fetch(`/api/cases/${encodeURIComponent(caseId)}/trace`);
  return (await res.json()) as TraceResponse;
}

export function AttestationTracePanel({ caseId }: { caseId: string }) {
  const [data, setData] = useState<TraceResponse | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    fetchCaseTrace(caseId)
      .then((json) => {
        if (!cancelled) setData(json);
      })
      .catch(() => {
        if (!cancelled) setData({ error: "Failed to load trace" });
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [caseId]);

  function refresh() {
    setLoading(true);
    void fetchCaseTrace(caseId)
      .then(setData)
      .catch(() => setData({ error: "Failed to load trace" }))
      .finally(() => setLoading(false));
  }

  return (
    <section className="rounded-lg border border-border bg-card/80 shadow-sm">
      <header className="flex items-center justify-between border-b border-border px-3 py-2">
        <div className="flex items-center gap-2">
          <FileWarning className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">Attestation trace</h2>
          <span className="text-[10px] text-muted-foreground">Phase 7 · workbench-api</span>
        </div>
        <button
          type="button"
          onClick={refresh}
          className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-[10px] text-muted-foreground hover:bg-secondary"
        >
          <RefreshCw className={cn("h-3 w-3", loading && "animate-spin")} />
          Refresh
        </button>
      </header>
      <div className="p-3">
        {loading && !data ? (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            Loading envelopes for case…
          </div>
        ) : data?.error ? (
          <p className="text-xs text-amber-600 dark:text-amber-400">{data.error}</p>
        ) : !data?.steps?.length ? (
          <p className="text-xs text-muted-foreground">
            No attestation rows found for <code className="rounded bg-secondary px-1">{caseId}</code>.
            Ensure orchestrator writes to the same <code className="rounded bg-secondary px-1">ATTEST_LOG_PATH</code> as
            workbench-api.
          </p>
        ) : (
          <ol className="space-y-2">
            {data.steps.map((s, i) => (
              <li
                key={`${s.agent_action_id}-${i}`}
                className="rounded-md border border-border/60 bg-background/50 px-2 py-1.5 text-xs"
              >
                <div className="flex flex-wrap items-center gap-2 font-medium text-foreground">
                  <span className="rounded bg-secondary px-1 py-0.5 font-mono text-[10px]">{s.kind}</span>
                  <span className="text-muted-foreground">{s.agent_id}</span>
                  <span className="text-muted-foreground">{s.execution_path}</span>
                  <span className="ml-auto lowercase text-primary">{s.verdict}</span>
                </div>
                <div className="mt-1 text-[10px] text-muted-foreground">
                  tools {s.tool_call_count} · beliefs {s.belief_count} · action{" "}
                  <code className="rounded bg-secondary/80 px-0.5">{s.agent_action_id}</code>
                </div>
              </li>
            ))}
          </ol>
        )}
      </div>
    </section>
  );
}
