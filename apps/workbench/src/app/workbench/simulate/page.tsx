"use client";

import { useState, useCallback } from "react";
import { Activity, AlertTriangle, CheckCircle2, ChevronRight, Cpu, FlaskConical, Radio, ShieldAlert, ShieldCheck, Zap } from "lucide-react";
import { cn } from "@/lib/utils";
import { SCENARIOS } from "@/lib/scenarios";
import type { ScenarioParams, Scenario } from "@/lib/scenarios";
import { ClassifierEvidencePanel } from "@/components/workbench/classifier-evidence-panel";
import type { FeatureImpact } from "@/lib/mock-data";
import type { SimulateResult } from "@/app/api/simulate/route";

// ── Pipeline step definitions ──────────────────────────────────────────────

const STEPS = [
  { id: "generate", label: "Generated",  icon: Zap },
  { id: "ingest",   label: "Collected",  icon: Radio },
  { id: "kafka",    label: "Kafka",      icon: Activity },
  { id: "triage",   label: "Triaged",    icon: Cpu },
  { id: "attested", label: "Attested",   icon: ShieldCheck },
] as const;

type StepId = typeof STEPS[number]["id"];
type RunState = "idle" | "running" | "done" | "error";

// ── Helpers ────────────────────────────────────────────────────────────────

const SEVERITY_COLORS: Record<Scenario["severity"], string> = {
  critical: "bg-red-500/15 text-red-400 border-red-500/30",
  high:     "bg-orange-500/15 text-orange-400 border-orange-500/30",
  medium:   "bg-yellow-500/15 text-yellow-400 border-yellow-500/30",
  low:      "bg-blue-500/15 text-blue-400 border-blue-500/30",
};

const VERDICT_COLORS: Record<string, string> = {
  suspicious: "bg-orange-500/15 text-orange-300 border-orange-500/30",
  malicious:  "bg-red-500/15 text-red-300 border-red-500/30",
  benign:     "bg-emerald-500/15 text-emerald-300 border-emerald-500/30",
  escalated:  "bg-purple-500/15 text-purple-300 border-purple-500/30",
  unknown:    "bg-muted/40 text-muted-foreground border-border",
};

function shapToFeatureImpacts(
  shapValues: Record<string, number>,
  inputFeatures: Record<string, number>,
): FeatureImpact[] {
  return Object.entries(shapValues)
    .sort((a, b) => Math.abs(b[1]) - Math.abs(a[1]))
    .slice(0, 8)
    .map(([name, impact]) => ({
      name: name.replace(/_/g, " "),
      value: (inputFeatures[name] ?? 0).toFixed(3),
      impact,
    }));
}

function ConfidenceBar({ value, label }: { value: number; label: string }) {
  const pct = Math.round(value * 100);
  const color = value > 0.7 ? "bg-severity-high" : value > 0.4 ? "bg-yellow-500" : "bg-signal-good";
  return (
    <div>
      <div className="mb-1 flex items-center justify-between text-xs">
        <span className="text-muted-foreground">{label}</span>
        <span className="font-mono font-medium">{pct}%</span>
      </div>
      <div className="h-1.5 rounded-full bg-secondary">
        <div className={cn("h-1.5 rounded-full transition-all duration-700", color)} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────

export default function SimulatePage() {
  const [selected, setSelected] = useState<Scenario>(SCENARIOS[0]);
  const [params, setParams]     = useState<ScenarioParams>(SCENARIOS[0].defaultParams);
  const [runState, setRunState] = useState<RunState>("idle");
  const [completedSteps, setCompletedSteps] = useState<Set<StepId>>(new Set());
  const [result, setResult]     = useState<SimulateResult | null>(null);
  const [error, setError]       = useState<string | null>(null);

  const selectScenario = useCallback((s: Scenario) => {
    setSelected(s);
    setParams(s.defaultParams);
    setRunState("idle");
    setCompletedSteps(new Set());
    setResult(null);
    setError(null);
  }, []);

  const runSimulation = useCallback(async () => {
    setRunState("running");
    setCompletedSteps(new Set());
    setResult(null);
    setError(null);

    // Step 1: mark "Generated" immediately
    setCompletedSteps(new Set<StepId>(["generate"]));

    try {
      const res = await fetch("/api/simulate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ scenario_id: selected.id, params }),
      });

      const data: SimulateResult = await res.json();

      // Animate remaining steps with short delays
      await delay(250);
      setCompletedSteps(new Set<StepId>(["generate", "ingest"]));
      await delay(300);
      setCompletedSteps(new Set<StepId>(["generate", "ingest", "kafka"]));
      await delay(300);
      setCompletedSteps(new Set<StepId>(["generate", "ingest", "kafka", "triage"]));
      await delay(250);
      setCompletedSteps(new Set<StepId>(["generate", "ingest", "kafka", "triage", "attested"]));

      setResult(data);
      setRunState("done");
    } catch (err) {
      setError(String(err));
      setRunState("error");
    }
  }, [selected, params]);

  const featureImpacts: FeatureImpact[] | null =
    result?.classifier_evidence?.shap_values
      ? shapToFeatureImpacts(
          result.classifier_evidence.shap_values,
          result.classifier_evidence.input_features ?? {},
        )
      : null;

  return (
    <div className="flex h-full min-h-0 flex-col gap-0">
      {/* ── Header ── */}
      <header className="flex items-center gap-3 border-b border-border/80 px-6 py-4">
        <FlaskConical className="size-5 text-primary" />
        <div>
          <h1 className="text-sm font-semibold">Simulation Lab</h1>
          <p className="text-xs text-muted-foreground">Inject realistic threat events and watch them traverse the live pipeline</p>
        </div>
      </header>

      <div className="flex min-h-0 flex-1 overflow-hidden">
        {/* ── Left: Scenario picker ── */}
        <aside className="w-72 shrink-0 overflow-y-auto border-r border-border/80 p-3">
          <p className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.15em] text-muted-foreground">Scenarios</p>
          <div className="space-y-1.5">
            {SCENARIOS.map((s) => (
              <button
                key={s.id}
                onClick={() => selectScenario(s)}
                className={cn(
                  "w-full rounded-md border px-3 py-2.5 text-left transition-colors",
                  selected.id === s.id
                    ? "border-primary/40 bg-primary/10"
                    : "border-transparent bg-card/60 hover:border-border hover:bg-card",
                )}
              >
                <div className="flex items-start justify-between gap-2">
                  <span className="text-xs font-medium leading-snug">{s.title}</span>
                  <span className={cn("shrink-0 rounded border px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide", SEVERITY_COLORS[s.severity])}>
                    {s.severity}
                  </span>
                </div>
                <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground line-clamp-2">{s.description}</p>
                <span className="mt-1.5 inline-block rounded bg-secondary px-1.5 py-0.5 font-mono text-[9px] text-muted-foreground">
                  {s.mitre}
                </span>
              </button>
            ))}
          </div>
        </aside>

        {/* ── Centre: Params + Run ── */}
        <div className="flex w-72 shrink-0 flex-col gap-4 overflow-y-auto border-r border-border/80 p-4">
          <div>
            <p className="mb-3 text-[10px] font-semibold uppercase tracking-[0.15em] text-muted-foreground">Parameters</p>
            <div className="space-y-3">
              <ParamField
                label="Actor username"
                value={params.username}
                onChange={(v) => setParams((p) => ({ ...p, username: v }))}
              />
              <ParamField
                label="Source IP"
                value={params.source_ip}
                onChange={(v) => setParams((p) => ({ ...p, source_ip: v }))}
              />
              <ParamField
                label="Region"
                value={params.region}
                onChange={(v) => setParams((p) => ({ ...p, region: v }))}
              />
              <div>
                <label className="mb-1.5 flex items-center justify-between text-xs text-muted-foreground">
                  <span>Severity score</span>
                  <span className="font-mono text-foreground">{params.severity.toFixed(2)}</span>
                </label>
                <input
                  type="range" min={0} max={1} step={0.01}
                  value={params.severity}
                  onChange={(e) => setParams((p) => ({ ...p, severity: parseFloat(e.target.value) }))}
                  className="w-full accent-primary"
                />
              </div>
            </div>
          </div>

          {/* Scenario detail */}
          <div className="rounded-md border border-border/60 bg-card/60 p-3 text-xs">
            <p className="mb-1 font-medium">{selected.title}</p>
            <p className="leading-relaxed text-muted-foreground">{selected.description}</p>
            <div className="mt-2 flex flex-wrap gap-1.5">
              <span className="rounded bg-secondary px-1.5 py-0.5 font-mono text-[10px]">{selected.mitre}</span>
              <span className="rounded bg-secondary px-1.5 py-0.5 text-[10px] text-muted-foreground">{selected.source}</span>
            </div>
          </div>

          <button
            onClick={runSimulation}
            disabled={runState === "running"}
            className={cn(
              "flex w-full items-center justify-center gap-2 rounded-md py-2.5 text-sm font-semibold transition-all",
              runState === "running"
                ? "cursor-not-allowed bg-primary/40 text-primary-foreground/60"
                : "bg-primary text-primary-foreground hover:bg-primary/90 active:scale-[0.98]",
            )}
          >
            {runState === "running" ? (
              <>
                <span className="size-3.5 animate-spin rounded-full border-2 border-current border-t-transparent" />
                Running…
              </>
            ) : (
              <>
                <FlaskConical className="size-3.5" />
                Run Simulation
              </>
            )}
          </button>
        </div>

        {/* ── Right: Pipeline viz + Results ── */}
        <div className="flex-1 overflow-y-auto p-5">
          {/* Pipeline */}
          <div className="mb-6">
            <p className="mb-3 text-[10px] font-semibold uppercase tracking-[0.15em] text-muted-foreground">Pipeline</p>
            <div className="flex items-center gap-0">
              {STEPS.map((step, i) => {
                const done    = completedSteps.has(step.id);
                const active  = runState === "running" && !done && completedSteps.size === i;
                const Icon    = step.icon;
                return (
                  <div key={step.id} className="flex items-center">
                    <div className={cn(
                      "flex flex-col items-center gap-1.5 rounded-lg border px-3 py-2.5 text-center transition-all duration-500",
                      done   ? "border-primary/40 bg-primary/10 text-primary" :
                      active ? "border-border bg-card/80 text-foreground animate-pulse" :
                               "border-border/40 bg-card/40 text-muted-foreground",
                    )}>
                      <Icon className="size-4" />
                      <span className="text-[10px] font-medium">{step.label}</span>
                      {done && <span className="text-[9px] text-primary/70">✓</span>}
                    </div>
                    {i < STEPS.length - 1 && (
                      <ChevronRight className={cn(
                        "mx-1 size-4 shrink-0 transition-colors duration-500",
                        completedSteps.has(STEPS[i + 1].id) ? "text-primary" : "text-border",
                      )} />
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          {/* Results */}
          {runState === "idle" && (
            <div className="flex h-48 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border/60 text-muted-foreground">
              <FlaskConical className="size-8 opacity-30" />
              <p className="text-sm">Select a scenario and click Run Simulation</p>
            </div>
          )}

          {runState === "error" && (
            <div className="flex items-start gap-3 rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-400">
              <AlertTriangle className="mt-0.5 size-4 shrink-0" />
              <div>
                <p className="font-medium">Simulation failed</p>
                <p className="mt-0.5 font-mono text-xs opacity-80">{error}</p>
              </div>
            </div>
          )}

          {(runState === "running" || runState === "done") && result === null && (
            <div className="space-y-3">
              {[...Array(3)].map((_, i) => (
                <div key={i} className="h-14 animate-pulse rounded-lg bg-card/60" />
              ))}
            </div>
          )}

          {result && (
            <div className="space-y-4">
              {/* Verdict card */}
              <div className="rounded-lg border border-border/80 bg-card/80 p-4">
                <div className="flex items-start justify-between gap-4">
                  <div className="flex items-center gap-3">
                    {result.verdict === "benign" ? (
                      <CheckCircle2 className="size-5 text-emerald-400" />
                    ) : (
                      <ShieldAlert className="size-5 text-orange-400" />
                    )}
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold capitalize">{result.verdict}</span>
                        <span className={cn("rounded border px-1.5 py-0.5 text-[10px] font-semibold uppercase", VERDICT_COLORS[result.verdict] ?? VERDICT_COLORS.unknown)}>
                          {result.verdict}
                        </span>
                        {result.escalated && (
                          <span className="rounded border border-purple-500/30 bg-purple-500/15 px-1.5 py-0.5 text-[10px] font-semibold text-purple-400">
                            ESCALATED
                          </span>
                        )}
                      </div>
                      <p className="mt-0.5 font-mono text-[10px] text-muted-foreground">
                        {result.execution_path} · {result.latency_ms}ms · action {result.action_id.slice(0, 8)}…
                      </p>
                    </div>
                  </div>
                  {/* Ingest status */}
                  <div className={cn(
                    "shrink-0 rounded border px-2 py-1 text-[10px] font-semibold",
                    result.ingest_ok
                      ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-400"
                      : "border-red-500/30 bg-red-500/10 text-red-400",
                  )}>
                    {result.ingest_ok ? `Kafka ✓ (${result.event_ids.length})` : "Kafka ✗"}
                  </div>
                </div>

                {result.escalation_reason && (
                  <p className="mt-2 rounded bg-purple-500/10 px-2 py-1.5 text-xs text-purple-300">
                    Escalation reason: {result.escalation_reason}
                  </p>
                )}
              </div>

              {/* Confidence bars */}
              <div className="rounded-lg border border-border/80 bg-card/80 p-4">
                <h3 className="mb-3 text-xs font-semibold">Confidence</h3>
                <div className="space-y-3">
                  <ConfidenceBar value={result.calibrated_confidence} label="Calibrated confidence" />
                  <ConfidenceBar value={result.novelty_score} label="Novelty score (OOD)" />
                </div>
              </div>

              {/* SHAP evidence */}
              {featureImpacts && featureImpacts.length > 0 && (
                <ClassifierEvidencePanel features={featureImpacts} />
              )}

              {/* Errors */}
              {(result.ingest_error || result.triage_error) && (
                <div className="space-y-2">
                  {result.ingest_error && (
                    <p className="rounded border border-yellow-500/30 bg-yellow-500/10 px-3 py-2 font-mono text-[11px] text-yellow-400">
                      Collector: {result.ingest_error}
                    </p>
                  )}
                  {result.triage_error && (
                    <p className="rounded border border-yellow-500/30 bg-yellow-500/10 px-3 py-2 font-mono text-[11px] text-yellow-400">
                      Orchestrator: {result.triage_error}
                    </p>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Helpers ────────────────────────────────────────────────────────────────

function ParamField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div>
      <label className="mb-1 block text-xs text-muted-foreground">{label}</label>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-md border border-border bg-card/60 px-2.5 py-1.5 font-mono text-xs text-foreground placeholder-muted-foreground focus:border-primary/60 focus:outline-none focus:ring-1 focus:ring-primary/20"
      />
    </div>
  );
}

function delay(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}
