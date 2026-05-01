"use client";

import type { ReactElement, ReactNode } from "react";
import { useMemo, useState } from "react";
import { Badge } from "@/components/reui/badge";
import {
  Stepper,
  StepperIndicator,
  StepperItem,
  StepperNav,
  StepperSeparator,
  StepperTitle,
  StepperTrigger,
} from "@/components/reui/stepper";
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "@/components/ui/hover-card";
import type { KafkaTraceStep } from "@/hooks/use-case-trace-ws";
import type { ExecutionPath, FeatureImpact } from "@/lib/mock-data";
import {
  BrainIcon,
  CheckIcon,
  CpuIcon,
  GavelIcon,
  LoaderCircleIcon,
  SearchIcon,
  ShieldIcon,
  WrenchIcon,
} from "lucide-react";
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

const TERMINAL_KINDS = new Set([
  "final_verdict",
  "envelope",
  "triage_complete",
]);

function prettyKind(kind: string): string {
  switch (kind) {
    case "intermediate_belief":
      return "Reasoning";
    case "tool_call":
      return "Tool call";
    case "final_verdict":
      return "Final verdict";
    case "envelope":
      return "Signed envelope";
    case "triage_complete":
      return "Triage complete";
    case "classifier_complete":
      return "Classifier complete";
    default:
      return kind.replaceAll("_", " ");
  }
}

function traceIcon(kind: string): ReactNode {
  switch (kind) {
    case "intermediate_belief":
      return <BrainIcon className="size-3.5" />;
    case "tool_call":
      return <WrenchIcon className="size-3.5" />;
    case "final_verdict":
      return <GavelIcon className="size-3.5" />;
    case "envelope":
      return <ShieldIcon className="size-3.5" />;
    case "classifier_complete":
      return <CpuIcon className="size-3.5" />;
    default:
      return <SearchIcon className="size-3.5" />;
  }
}

function traceBadgeVariant(
  kind: string,
): React.ComponentProps<typeof Badge>["variant"] {
  if (kind === "tool_call") return "primary-light";
  if (kind === "intermediate_belief") return "secondary";
  if (TERMINAL_KINDS.has(kind)) return "success-light";
  return "secondary";
}

function TraceStepBody({ row }: { row: Row }): ReactElement {
  const [open, setOpen] = useState(false);
  return (
    <div className="min-w-0 flex-1 pb-5 text-left">
      <div className="flex flex-wrap items-center gap-2">
        <StepperTitle className="text-sm font-semibold capitalize">
          {prettyKind(row.kind)}
        </StepperTitle>
        <Badge size="sm" variant={traceBadgeVariant(row.kind)}>
          {row.kind}
        </Badge>
        <span className="font-mono text-[10px] text-muted-foreground">
          {row.agent_id}
        </span>
        <span className="font-mono text-[10px] text-muted-foreground">
          {row.execution_path}
        </span>
        {row.expandable ? (
          <button
            type="button"
            className="ml-auto rounded px-1.5 py-0.5 text-[10px] text-primary hover:bg-primary/10"
            onClick={() => setOpen(!open)}
          >
            {open ? "Collapse" : "Expand"}
          </button>
        ) : null}
      </div>
      <div className="mt-2 text-sm leading-6 text-muted-foreground">
        {row.expandable && open ? (
          <div className="flex flex-wrap gap-1">
            {renderWithCitations(row.summary)}
          </div>
        ) : (
          <span data-customer-data className="line-clamp-3">
            {row.summary}
          </span>
        )}
      </div>
      <div className="mt-2 font-mono text-[10px] text-muted-foreground/70">
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
    const live: Row[] = liveSteps
      .filter((s) => s.step_kind !== "classifier_complete")
      .map((s) => ({
        key: `live-${s.agent_action_id}-${s.step_kind}-${s.ts}`,
        kind: s.step_kind,
        summary: s.summary,
        agent_id: s.agent_id,
        execution_path: s.execution_path,
        agent_action_id: s.agent_action_id,
        expandable:
          s.step_kind === "tool_call" || s.step_kind === "intermediate_belief",
      }));
    const http: Row[] = httpSteps
      .filter((s) => s.kind !== "classifier_complete")
      .map((s, index) => ({
        key: `http-${s.agent_action_id}-${s.kind}-${index}`,
        kind: s.kind,
        summary: s.verdict,
        agent_id: s.agent_id,
        execution_path: s.execution_path,
        agent_action_id: s.agent_action_id,
        expandable: s.kind === "tool_call" || s.kind === "intermediate_belief",
      }));
    const seen = new Set<string>();
    const out: Row[] = [];
    for (const r of [...http, ...live]) {
      const fingerprint = `${r.agent_action_id}:${r.kind}:${r.summary}`;
      if (seen.has(fingerprint)) continue;
      seen.add(fingerprint);
      out.push(r);
    }
    return out;
  }, [httpSteps, liveSteps]);

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

  const activeStep = rows.length;
  const terminalComplete = TERMINAL_KINDS.has(
    rows[rows.length - 1]?.kind ?? "",
  );

  return (
    <div
      className="max-h-[min(34rem,58vh)] overflow-auto rounded-md border border-border/60 bg-background/40 p-4"
      data-testid="llm-reasoning-timeline"
    >
      <Stepper
        value={activeStep}
        orientation="vertical"
        indicators={{
          completed: <CheckIcon className="size-3.5" />,
          loading: <LoaderCircleIcon className="size-3.5 animate-spin" />,
        }}
      >
        <StepperNav className="w-full">
          {rows.map((row, index) => {
            const step = index + 1;
            const isLast = index === rows.length - 1;
            return (
              <StepperItem
                key={row.key}
                step={step}
                completed={!isLast || terminalComplete}
                className="relative w-full items-start not-last:flex-1"
                data-testid="trace-step"
                data-kind={row.kind}
              >
                <StepperTrigger
                  className="w-full items-start gap-3 pb-0 text-left"
                  asChild
                >
                  <div className="flex w-full items-start gap-3">
                    <StepperIndicator className="mt-0.5 size-7 border border-border bg-background data-[state=active]:bg-primary data-[state=completed]:bg-success data-[state=completed]:text-white">
                      {traceIcon(row.kind)}
                    </StepperIndicator>
                    <TraceStepBody row={row} />
                  </div>
                </StepperTrigger>
                {!isLast ? (
                  <StepperSeparator className="absolute inset-y-0 left-3.5 top-8 -order-1 m-0 -translate-x-1/2 group-data-[orientation=vertical]/stepper-nav:h-[calc(100%-2.25rem)] group-data-[state=completed]/step:bg-success" />
                ) : null}
              </StepperItem>
            );
          })}
        </StepperNav>
      </Stepper>
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
        <h3 className="mb-2 text-xs font-medium text-muted-foreground">
          LLM &amp; tool steps
        </h3>
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
