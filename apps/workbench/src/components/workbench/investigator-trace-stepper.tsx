"use client";

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
import type { KafkaTraceStep } from "@/hooks/use-case-trace-ws";
import type { ExecutionPath } from "@/lib/mock-data";
import {
  CheckIcon,
  BrainIcon,
  LoaderCircleIcon,
  WrenchIcon,
  GavelIcon,
  DatabaseIcon,
  CpuIcon,
  SearchIcon,
  ShieldIcon,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

function normPath(p: string): string {
  return p.toLowerCase();
}

function phaseOf(kind: string): number {
  if (kind === "intermediate_belief") return 1;
  if (kind === "tool_call") return 2;
  if (kind === "final_verdict" || kind === "envelope" || kind === "triage_complete") return 3;
  return 0;
}

const PIPELINE_STEPS = [
  {
    title: "Reasoning",
    description: "Intermediate beliefs from the investigator model.",
    icon: <BrainIcon className="size-4" />,
  },
  {
    title: "Tools & evidence",
    description: "MCP / warm-tier queries and policy-checked calls.",
    icon: <WrenchIcon className="size-4" />,
  },
  {
    title: "Verdict & attestation",
    description: "Final verdict, guardrails, signed envelope.",
    icon: <GavelIcon className="size-4" />,
  },
] as const;

function deriveStepState(llmSteps: KafkaTraceStep[]) {
  if (llmSteps.length === 0) {
    return {
      activeStep: 1,
      loadingStep: 1 as number | null,
      step1Done: false,
      step2Done: false,
      step3Done: false,
    };
  }
  const sorted = [...llmSteps].sort((a, b) => a.ts - b.ts);
  const last = sorted[sorted.length - 1]!;
  const lk = last.step_kind;

  const step1Done = sorted.some((s) => phaseOf(s.step_kind) >= 2);
  const step2Done = sorted.some((s) =>
    ["final_verdict", "envelope", "triage_complete"].includes(s.step_kind),
  );
  const step3Done = sorted.some((s) =>
    ["envelope", "triage_complete"].includes(s.step_kind),
  );

  let activeStep = 1;
  let loadingStep: number | null = 1;

  if (step3Done) {
    activeStep = 3;
    loadingStep = null;
  } else if (lk === "final_verdict") {
    activeStep = 3;
    loadingStep = 3;
  } else if (lk === "tool_call") {
    activeStep = 2;
    loadingStep = 2;
  } else if (lk === "intermediate_belief") {
    activeStep = 1;
    loadingStep = 1;
  } else {
    activeStep = Math.min(3, Math.max(1, phaseOf(lk) || 1));
    loadingStep = activeStep;
  }

  return {
    activeStep,
    loadingStep,
    step1Done,
    step2Done,
    step3Done,
  };
}

// ─────────────────────────────────────────────────────────────────────────────
// TriagePipelineStepper — full centered stepper shown while triage is in flight
// ─────────────────────────────────────────────────────────────────────────────

const PIPELINE_4 = [
  {
    title: "Event ingested",
    description: "OCSF parsed · baseline fetched",
    icon: <DatabaseIcon className="size-4" />,
  },
  {
    title: "Classifier",
    description: "XGBoost/ONNX · SHAP attribution",
    icon: <CpuIcon className="size-4" />,
  },
  {
    title: "LLM investigation",
    description: "Investigator agent · MCP tools",
    icon: <BrainIcon className="size-4" />,
  },
  {
    title: "Verdict & attestation",
    description: "Guardrails · Ed25519 envelope",
    icon: <GavelIcon className="size-4" />,
  },
] as const;

function formatElapsed(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

// ── Step kind metadata ────────────────────────────────────────────────────────

const STEP_META: Record<string, { label: string; icon: React.ReactNode; color: string }> = {
  tool_call: {
    label: "Tool call",
    icon: <WrenchIcon className="size-3" />,
    color: "text-blue-400 bg-blue-500/10 border-blue-500/20",
  },
  intermediate_belief: {
    label: "Reasoning",
    icon: <BrainIcon className="size-3" />,
    color: "text-violet-400 bg-violet-500/10 border-violet-500/20",
  },
  classifier_complete: {
    label: "Classifier",
    icon: <CpuIcon className="size-3" />,
    color: "text-emerald-400 bg-emerald-500/10 border-emerald-500/20",
  },
  final_verdict: {
    label: "Verdict",
    icon: <GavelIcon className="size-3" />,
    color: "text-amber-400 bg-amber-500/10 border-amber-500/20",
  },
  envelope: {
    label: "Attested",
    icon: <ShieldIcon className="size-3" />,
    color: "text-signal-good bg-signal-good/10 border-signal-good/20",
  },
};

function stepMeta(kind: string) {
  return STEP_META[kind] ?? {
    label: kind,
    icon: <SearchIcon className="size-3" />,
    color: "text-muted-foreground bg-secondary border-border",
  };
}

// ── Live trace log ────────────────────────────────────────────────────────────

function LiveTraceLog({ steps }: { steps: KafkaTraceStep[] }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sorted = useMemo(
    () => [...steps].sort((a, b) => a.ts - b.ts),
    [steps],
  );

  // Auto-scroll to bottom of the log container ONLY (never affects parent page).
  // Using scrollTop on the ref — scrollIntoView() would bubble up and scroll
  // the whole page, making navigation feel frozen.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [steps.length]);

  if (sorted.length === 0) {
    return (
      <div className="flex items-center gap-2 rounded-md border border-border/50 bg-secondary/20 px-3 py-2 text-xs text-muted-foreground/60">
        <LoaderCircleIcon className="size-3.5 animate-spin shrink-0" />
        Waiting for trace events…
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className="max-h-48 overflow-y-auto rounded-md border border-border bg-background/60 p-1 space-y-0.5 text-xs"
    >
      {sorted.map((s, i) => {
        const m = stepMeta(s.step_kind);
        return (
          <div
            key={`${s.step_kind}-${s.ts}-${i}`}
            className="flex items-start gap-2 rounded px-2 py-1.5 hover:bg-secondary/40 transition-colors"
          >
            {/* Kind badge */}
            <span
              className={`mt-px inline-flex shrink-0 items-center gap-1 rounded border px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wide leading-none ${m.color}`}
            >
              {m.icon}
              {m.label}
            </span>
            {/* Summary */}
            <span className="min-w-0 leading-5 text-muted-foreground line-clamp-2" data-customer-data>
              {s.summary || "—"}
            </span>
            {/* Relative time */}
            <span className="ml-auto shrink-0 font-mono text-[10px] text-muted-foreground/50 tabular-nums">
              +{((s.ts - sorted[0]!.ts) / 1000).toFixed(1)}s
            </span>
          </div>
        );
      })}
    </div>
  );
}

export function TriagePipelineStepper({
  liveSteps,
  caseId,
}: {
  liveSteps: KafkaTraceStep[];
  caseId: string;
}) {
  // Persist start time in sessionStorage so the timer survives navigation away and back.
  const [startedAt] = useState<number>(() => {
    const key = `triage_start:${caseId}`;
    try {
      const stored = sessionStorage.getItem(key);
      if (stored) return parseInt(stored, 10);
    } catch { /* SSR / private browsing — ignore */ }
    const now = Date.now();
    try { sessionStorage.setItem(key, String(now)); } catch { /* ignore */ }
    return now;
  });

  // Initialize to 0 so server and client render the same initial HTML.
  // The real elapsed value is set immediately in the effect below (client-only).
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    setElapsed(Date.now() - startedAt);
    const id = setInterval(() => {
      setElapsed(Date.now() - startedAt);
    }, 200);
    return () => clearInterval(id);
  }, [startedAt]);

  // Derive step states from WS trace steps.
  const llmSteps = liveSteps.filter((s) => {
    const p = s.execution_path.toLowerCase();
    return p === "llm" || p === "hybrid";
  });
  const hasLlmSteps = llmSteps.length > 0;
  const hasFinalVerdict = liveSteps.some((s) =>
    s.step_kind === "final_verdict" || s.step_kind === "triage_complete",
  );
  const hasEnvelope = liveSteps.some((s) =>
    s.step_kind === "envelope" || s.step_kind === "triage_complete",
  );
  // Real classifier_complete trace step emitted by orchestrator after ONNX run.
  const hasClassifierStep = liveSteps.some((s) => s.step_kind === "classifier_complete");

  // Timer fallback kept for degraded WS or non-LLM (classifier-only) paths.
  const classifierLikelyDone = hasClassifierStep || elapsed > 5_000;

  // Step 1: always done  Step 2: active→done  Step 3: pending→active→done  Step 4: pending→done
  const step1Done = true;
  const step2Done = hasLlmSteps || hasEnvelope || classifierLikelyDone;
  const step3Done = hasEnvelope || hasFinalVerdict;
  const step4Done = hasEnvelope;

  // Compute active/loading from time-aware flags
  const activeStep = step4Done ? 4 : step3Done ? 4 : step2Done ? 3 : 2;
  const loadingStep = step4Done ? null : step3Done ? 4 : step2Done ? 3 : 2;

  // Clean up sessionStorage once we know triage finished
  useEffect(() => {
    if (step4Done) {
      try { sessionStorage.removeItem(`triage_start:${caseId}`); } catch { /* ignore */ }
    }
  }, [step4Done, caseId]);

  const doneFlags = [step1Done, step2Done, step3Done, step4Done] as const;

  return (
    <div className="flex min-h-[320px] flex-col items-center justify-center gap-8 py-8 px-4">
      {/* Header */}
      <div className="flex flex-col items-center gap-2 text-center">
        <div className="relative grid h-14 w-14 place-items-center rounded-full border-2 border-primary/30 bg-primary/10">
          <LoaderCircleIcon className="size-7 animate-spin text-primary" aria-hidden />
        </div>
        <p className="text-base font-semibold tracking-tight">Analysing case</p>
        <p className="font-mono text-xs text-muted-foreground">
          {formatElapsed(elapsed)} elapsed
        </p>
      </div>

      {/* 4-step pipeline */}
      <Stepper
        value={activeStep}
        indicators={{
          completed: <CheckIcon className="size-3.5" />,
          loading: <LoaderCircleIcon className="size-3.5 animate-spin" />,
        }}
        className="w-full max-w-2xl"
      >
        <StepperNav className="gap-0">
          {PIPELINE_4.map((step, index) => {
            const n = index + 1;
            const done = doneFlags[index] ?? false;
            const loading = loadingStep === n;
            return (
              <StepperItem
                key={step.title}
                step={n}
                completed={done}
                loading={loading}
                className="relative flex-1 items-start"
              >
                <StepperTrigger asChild>
                  <div className="flex flex-col items-center gap-2 px-1 text-center">
                    <StepperIndicator className="data-[state=inactive]:border-border data-[state=inactive]:text-muted-foreground data-[state=completed]:bg-success size-9 shrink-0 border-2 data-[state=completed]:text-white data-[state=inactive]:bg-transparent">
                      {step.icon}
                    </StepperIndicator>
                    <div className="flex flex-col items-center gap-1">
                      <StepperTitle className="group-data-[state=inactive]/step:text-muted-foreground text-center text-xs font-semibold sm:text-sm">
                        {step.title}
                      </StepperTitle>
                      <p className="hidden text-[10px] leading-snug text-muted-foreground sm:block">
                        {step.description}
                      </p>
                      <Badge
                        size="sm"
                        variant="primary-light"
                        className="hidden group-data-[state=active]/step:inline-flex"
                      >
                        In progress
                      </Badge>
                      <Badge
                        variant="success-light"
                        size="sm"
                        className="hidden group-data-[state=completed]/step:inline-flex"
                      >
                        Done
                      </Badge>
                      <Badge
                        variant="secondary"
                        size="sm"
                        className="text-muted-foreground hidden group-data-[state=inactive]/step:inline-flex"
                      >
                        Pending
                      </Badge>
                    </div>
                  </div>
                </StepperTrigger>

                {PIPELINE_4.length > index + 1 && (
                  <StepperSeparator className="group-data-[state=completed]/step:bg-success absolute inset-x-0 start-1/2 top-[18px] m-0 group-data-[orientation=horizontal]/stepper-nav:w-[calc(100%-2.25rem)] group-data-[orientation=horizontal]/stepper-nav:flex-none" />
                )}
              </StepperItem>
            );
          })}
        </StepperNav>
      </Stepper>

      {/* Live trace log — always show, expands as steps arrive */}
      <div className="w-full max-w-2xl space-y-1.5">
        {/* Header row */}
        <div className="flex items-center justify-between px-1">
          <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
            Live trace
          </span>
          {liveSteps.length > 0 && (
            <span className="font-mono text-[10px] text-muted-foreground/60">
              {liveSteps.length} event{liveSteps.length !== 1 ? "s" : ""}
            </span>
          )}
        </div>
        <LiveTraceLog steps={liveSteps} />
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// InvestigatorTraceStepper — compact stepper inside the evidence section
// ─────────────────────────────────────────────────────────────────────────────

export function InvestigatorTraceStepper({
  liveSteps,
  executionPath,
  verdict,
}: {
  liveSteps: KafkaTraceStep[];
  executionPath: ExecutionPath;
  /** Orchestrator verdict (e.g. `needs_investigation`); may not match mock `CaseRecord` unions. */
  verdict: string;
}) {
  const llmSteps = useMemo(
    () =>
      liveSteps.filter((s) => {
        const p = normPath(s.execution_path);
        return p === "llm" || p === "hybrid";
      }),
    [liveSteps],
  );

  const investigatorRelevant =
    verdict === "needs_investigation" ||
    executionPath === "llm" ||
    llmSteps.length > 0;
  if (!investigatorRelevant) return null;

  const { activeStep, loadingStep, step1Done, step2Done, step3Done } =
    deriveStepState(llmSteps);

  const doneFlags = [
    step1Done || step3Done,
    step2Done || step3Done,
    step3Done,
  ] as const;

  return (
    <div
      className="mb-4 rounded-lg border border-border bg-card/60 p-3"
      data-testid="investigator-trace-stepper"
    >
      <div className="mb-3 text-xs font-medium text-muted-foreground">
        Investigator pipeline
        {llmSteps.length === 0 ? (
          <span className="ml-2 font-mono text-[10px] text-muted-foreground/80">
            — no LLM trace steps yet
          </span>
        ) : null}
      </div>
      <Stepper
        value={activeStep}
        indicators={{
          completed: <CheckIcon className="size-3.5" />,
          loading: <LoaderCircleIcon className="size-3.5 animate-spin" />,
        }}
        className="w-full space-y-3"
      >
        <StepperNav className="gap-2 sm:gap-3">
          {PIPELINE_STEPS.map((step, index) => {
            const n = index + 1;
            return (
              <StepperItem
                key={step.title}
                step={n}
                completed={doneFlags[index] ?? false}
                loading={loadingStep === n}
                className="relative flex-1 items-start"
              >
                <StepperTrigger
                  className="flex grow flex-col items-start justify-center gap-2 sm:gap-2.5"
                  asChild
                >
                  <div className="flex flex-col items-start gap-2 sm:flex-row sm:gap-2.5">
                    <StepperIndicator className="data-[state=inactive]:border-border data-[state=inactive]:text-muted-foreground data-[state=completed]:bg-success size-8 shrink-0 border-2 data-[state=completed]:text-white data-[state=inactive]:bg-transparent">
                      {step.icon}
                    </StepperIndicator>
                    <div className="flex min-w-0 flex-col items-start gap-1">
                      <div className="text-muted-foreground text-[10px] font-semibold uppercase">
                        Step {n}
                      </div>
                      <StepperTitle className="group-data-[state=inactive]/step:text-muted-foreground text-start text-sm font-semibold sm:text-base">
                        {step.title}
                      </StepperTitle>
                      <p className="hidden text-[11px] text-muted-foreground sm:block">
                        {step.description}
                      </p>
                      <div>
                        <Badge
                          size="sm"
                          variant="primary-light"
                          className="hidden group-data-[state=active]/step:inline-flex"
                        >
                          In progress
                        </Badge>
                        <Badge
                          variant="success-light"
                          size="sm"
                          className="hidden group-data-[state=completed]/step:inline-flex"
                        >
                          Completed
                        </Badge>
                        <Badge
                          variant="secondary"
                          size="sm"
                          className="text-muted-foreground hidden group-data-[state=inactive]/step:inline-flex"
                        >
                          Pending
                        </Badge>
                      </div>
                    </div>
                  </div>
                </StepperTrigger>

                {PIPELINE_STEPS.length > index + 1 && (
                  <StepperSeparator className="group-data-[state=completed]/step:bg-success absolute inset-x-0 start-7 top-4 m-0 group-data-[orientation=horizontal]/stepper-nav:w-[calc(100%-1.75rem)] group-data-[orientation=horizontal]/stepper-nav:flex-none sm:start-9 sm:top-4" />
                )}
              </StepperItem>
            );
          })}
        </StepperNav>
      </Stepper>
    </div>
  );
}
