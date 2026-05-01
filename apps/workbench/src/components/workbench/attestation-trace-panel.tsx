"use client";

import { FileWarning, Loader2, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import type { KafkaTraceStep, WsTraceStatus } from "@/hooks/use-case-trace-ws";

type HttpTraceStep = {
  kind: string;
  agent_action_id: string;
  agent_id: string;
  execution_path: string;
  verdict: string;
  tool_call_count: number;
  belief_count: number;
};

type TraceResponse = { steps?: HttpTraceStep[]; error?: string };

async function fetchCaseTrace(caseId: string): Promise<TraceResponse> {
  const res = await fetch(`/api/cases/${encodeURIComponent(caseId)}/trace`);
  return (await res.json()) as TraceResponse;
}

function kafkaToDisplay(s: KafkaTraceStep): HttpTraceStep {
  return {
    kind: s.step_kind,
    agent_action_id: s.agent_action_id,
    agent_id: s.agent_id,
    execution_path: s.execution_path,
    verdict: s.summary,
    tool_call_count: 0,
    belief_count: 0,
  };
}

export function AttestationTracePanel({
  caseId,
  liveSteps = [],
  wsStatus = "disconnected",
}: {
  caseId: string;
  liveSteps?: KafkaTraceStep[];
  wsStatus?: WsTraceStatus;
}) {
  const [data, setData] = useState<TraceResponse | null>(null);
  const [loading, setLoading] = useState(true);

  const scrollRef = useRef<HTMLDivElement>(null);
  const stickTop = useRef(true);

  const load = useCallback(() => {
    setLoading(true);
    void fetchCaseTrace(caseId)
      .then(setData)
      .catch(() => setData({ error: "Failed to load trace" }))
      .finally(() => setLoading(false));
  }, [caseId]);

  useEffect(() => {
    queueMicrotask(() => {
      load();
    });
  }, [load]);

  const mergedSteps = useMemo(() => {
    const http = data?.steps ?? [];
    const live = liveSteps.map(kafkaToDisplay);
    const seen = new Set<string>();
    const out: HttpTraceStep[] = [];
    for (const s of [...live, ...http]) {
      const k = `${s.agent_action_id}-${s.kind}`;
      if (seen.has(k)) continue;
      seen.add(k);
      out.push(s);
    }
    return out;
  }, [data?.steps, liveSteps]);

  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el || !stickTop.current) return;
    el.scrollTop = 0;
  }, [mergedSteps.length]);

  return (
    <section className="rounded-lg border border-border bg-card/80 shadow-sm">
      <header className="flex items-center justify-between border-b border-border px-3 py-2">
        <div className="flex items-center gap-2">
          <FileWarning className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">Attestation trace</h2>
          <span className="text-[10px] text-muted-foreground">
            HTTP replay + live · WS {wsStatus}
          </span>
        </div>
        <button
          type="button"
          onClick={load}
          className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-[10px] text-muted-foreground hover:bg-secondary"
        >
          <RefreshCw className={cn("h-3 w-3", loading && "animate-spin")} />
          Refresh
        </button>
      </header>
      <div
        ref={scrollRef}
        className="max-h-[min(24rem,40vh)] overflow-auto p-3"
        onScroll={(e) => {
          stickTop.current = e.currentTarget.scrollTop <= 8;
        }}
      >
        {loading && !data ? (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            Loading envelopes for case…
          </div>
        ) : data?.error ? (
          <p className="text-xs text-amber-600 dark:text-amber-400">{data.error}</p>
        ) : !mergedSteps.length ? (
          <p className="text-xs text-muted-foreground">
            No attestation rows found for <code className="rounded bg-secondary px-1">{caseId}</code>.
          </p>
        ) : (
          <ol className="space-y-2">
            {mergedSteps.map((s, i) => (
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
