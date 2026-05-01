"use client";

import { useVirtualizer } from "@tanstack/react-virtual";
import type { ReactElement, ReactNode } from "react";
import { useMemo, useRef, useState } from "react";
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "@/components/ui/hover-card";
import type { KafkaTraceStep } from "@/hooks/use-case-trace-ws";
import type { ExecutionPath, FeatureImpact } from "@/lib/mock-data";
import { ClassifierEvidencePanel } from "./classifier-evidence-panel";

const EVIDENCE_REF = /\[evidence:([a-f0-9-]{36})\]/gi;

function CitationChip({ id }: { id: string }) {
  const [data, setData] = useState<{ api_operation?: string } | null>(null);
  const [loaded, setLoaded] = useState(false);

  function load() {
    if (loaded) return;
    setLoaded(true);
    void fetch(`/api/events?id=${encodeURIComponent(id)}`)
      .then((r) => r.json())
      .then((j) => setData(j))
      .catch(() => setData(null));
  }

  return (
    <HoverCard onOpenChange={(o) => o && load()}>
      <HoverCardTrigger asChild>
        <button
          type="button"
          className="rounded border border-border bg-secondary/60 px-1.5 py-0.5 font-mono text-[10px] text-primary hover:bg-secondary"
        >
          {id.slice(0, 8)}…
        </button>
      </HoverCardTrigger>
      <HoverCardContent className="w-72 text-xs">
        <p className="font-mono text-[10px] text-muted-foreground">{id}</p>
        {data?.api_operation ? (
          <p className="mt-2">{data.api_operation}</p>
        ) : (
          <p className="mt-2 text-muted-foreground">Event metadata</p>
        )}
      </HoverCardContent>
    </HoverCard>
  );
}

function renderWithCitations(text: string): ReactNode[] | string {
  const parts: ReactNode[] = [];
  let last = 0;
  let m: RegExpExecArray | null;
  const re = new RegExp(EVIDENCE_REF);
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) {
      parts.push(
        <span key={`t-${last}`} data-customer-data>
          {text.slice(last, m.index)}
        </span>,
      );
    }
    parts.push(<CitationChip key={m[1]} id={m[1]} />);
    last = m.index + m[0].length;
  }
  if (last < text.length) {
    parts.push(
      <span key={`t-${last}`} data-customer-data>
        {text.slice(last)}
      </span>,
    );
  }
  return parts.length ? parts : text;
}

export type HttpTraceStep = {
  kind: string;
  agent_action_id: string;
  agent_id: string;
  execution_path: string;
  verdict: string;
  tool_call_count: number;
  belief_count: number;
};

type Row = {
  key: string;
  kind: string;
  summary: string;
  agent_id: string;
  execution_path: string;
  agent_action_id: string;
  expandable: boolean;
};

function TimelineRow({ row, start }: { row: Row; start: number }): ReactElement {
  const [open, setOpen] = useState(false);
  return (
    <div
      data-testid="trace-step"
      data-kind={row.kind}
      className="absolute left-0 right-0 border-b border-border/40 px-2 py-2 text-xs"
      style={{ transform: `translateY(${start}px)` }}
    >
      <div className="flex flex-wrap items-center gap-2">
        <span className="rounded bg-secondary px-1 py-0.5 font-mono text-[10px] uppercase">
          {row.kind}
        </span>
        <span className="text-muted-foreground">{row.agent_id}</span>
        <span className="text-muted-foreground">{row.execution_path}</span>
        {row.expandable ? (
          <button
            type="button"
            className="ml-auto text-[10px] text-primary"
            onClick={() => setOpen(!open)}
          >
            {open ? "▾" : "▸"}
          </button>
        ) : null}
      </div>
      <div className="mt-1 text-muted-foreground">
        {row.expandable && open ? (
          <div className="flex flex-wrap gap-1">{renderWithCitations(row.summary)}</div>
        ) : (
          <span data-customer-data className="line-clamp-2">
            {row.summary}
          </span>
        )}
      </div>
      <div className="mt-0.5 font-mono text-[10px] text-muted-foreground/80">
        {row.agent_action_id}
      </div>
    </div>
  );
}

export function LLMReasoningTimeline({
  httpSteps,
  liveSteps,
}: {
  httpSteps: HttpTraceStep[];
  liveSteps: KafkaTraceStep[];
}): ReactElement {
  const rows = useMemo(() => {
    const live: Row[] = liveSteps.map((s) => ({
      key: `live-${s.agent_action_id}-${s.step_kind}-${s.ts}`,
      kind: s.step_kind,
      summary: s.summary,
      agent_id: s.agent_id,
      execution_path: s.execution_path,
      agent_action_id: s.agent_action_id,
      expandable: s.step_kind === "tool_call",
    }));
    const http: Row[] = httpSteps.map((s) => ({
      key: `http-${s.agent_action_id}-${s.kind}`,
      kind: s.kind,
      summary: `${s.verdict} · tools ${s.tool_call_count} · beliefs ${s.belief_count}`,
      agent_id: s.agent_id,
      execution_path: s.execution_path,
      agent_action_id: s.agent_action_id,
      expandable: false,
    }));
    const seen = new Set<string>();
    const out: Row[] = [];
    for (const r of [...live, ...http]) {
      if (seen.has(r.key)) continue;
      seen.add(r.key);
      out.push(r);
    }
    return out;
  }, [httpSteps, liveSteps]);

  const scrollRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 72,
    overscan: 12,
  });

  if (rows.length === 0) {
    return (
      <div
        className="rounded-md border border-border/60 bg-background/40 p-4 text-xs text-muted-foreground"
        data-testid="llm-reasoning-timeline-empty"
      >
        No LLM/tool trace rows are available for this case yet. Live WebSocket
        steps and attestation replay will appear here when present.
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="max-h-[min(28rem,50vh)] overflow-auto rounded-md border border-border/60 bg-background/40"
      data-testid="llm-reasoning-timeline"
    >
      <div style={{ height: `${virtualizer.getTotalSize()}px`, position: "relative" }}>
        {virtualizer.getVirtualItems().map((vi) => (
          <TimelineRow key={rows[vi.index].key} row={rows[vi.index]} start={vi.start} />
        ))}
      </div>
    </div>
  );
}

export function HybridDualEvidence({
  featureImpacts,
  httpSteps,
  liveSteps,
}: {
  featureImpacts: FeatureImpact[];
  httpSteps: HttpTraceStep[];
  liveSteps: KafkaTraceStep[];
}): ReactElement {
  return (
    <div className="grid gap-3 xl:grid-cols-2">
      <ClassifierEvidencePanel features={featureImpacts} />
      <div>
        <h3 className="mb-2 text-xs font-medium text-muted-foreground">LLM &amp; tool steps</h3>
        <LLMReasoningTimeline httpSteps={httpSteps} liveSteps={liveSteps} />
      </div>
    </div>
  );
}

export function ReasoningTraceViewer({
  executionPath,
  featureImpacts,
  httpSteps,
  liveSteps,
}: {
  executionPath: ExecutionPath;
  featureImpacts: FeatureImpact[];
  httpSteps: HttpTraceStep[];
  liveSteps: KafkaTraceStep[];
}): ReactElement {
  if (executionPath === "classifier") {
    return <ClassifierEvidencePanel features={featureImpacts} />;
  }
  if (executionPath === "llm") {
    return <LLMReasoningTimeline httpSteps={httpSteps} liveSteps={liveSteps} />;
  }
  return (
    <HybridDualEvidence
      featureImpacts={featureImpacts}
      httpSteps={httpSteps}
      liveSteps={liveSteps}
    />
  );
}
