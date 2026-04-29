"use client";

import { useState, useCallback, useEffect, useRef } from "react";
import {
  Activity, AlertTriangle, Archive, CheckCircle2, Cpu, Database,
  FlaskConical, Hourglass, Layers, Radio, Search,
  Shield, ShieldAlert, ShieldCheck, Zap,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { SCENARIOS } from "@/lib/scenarios";
import type { ScenarioParams, Scenario } from "@/lib/scenarios";
import { ClassifierEvidencePanel } from "@/components/workbench/classifier-evidence-panel";
import type { FeatureImpact } from "@/lib/mock-data";
import type { SimulateResponse, StageResult } from "@/app/api/simulate/route";
import type { VerifyResponse } from "@/app/api/simulate/verify/route";

// ── Stage definitions ─────────────────────────────────────────────────────

type StageKey =
  | "collector" | "orchestrator" | "calibration" | "attestation"
  | "risingwave" | "detection" | "iceberg" | "mcp";

type StageMeta = {
  key: StageKey;
  label: string;
  service: string;
  proves: string;
  icon: typeof Zap;
};

const HOT_PATH_STAGES: StageMeta[] = [
  { key: "collector",    label: "Collector",         service: "attest-collector :4000",   proves: "CloudTrail → OCSF normalized + published to Kafka",         icon: Radio },
  { key: "orchestrator", label: "Orchestrator",      service: "attest-orchestrator :4300", proves: "Hybrid triage loop (XGBoost ONNX + Mahalanobis novelty)", icon: Cpu },
  { key: "calibration",  label: "Calibration",       service: "calibration-sidecar :5001", proves: "Isotonic regression — raw score → calibrated probability", icon: Activity },
  { key: "attestation",  label: "Attestation",       service: "attest-attestation crate",  proves: "Ed25519 signed envelope appended to attestations.ndjson", icon: ShieldCheck },
];

const MEMORY_STAGES: StageMeta[] = [
  { key: "risingwave", label: "RisingWave",      service: "risingwave :4566",       proves: "Streaming MV update — user_baselines includes this event",   icon: Database },
  { key: "detection",  label: "Detection + ClickHouse", service: "detection-runtime + clickhouse :8123", proves: "Detection rule matched, fired alert stored in ClickHouse", icon: Search },
  { key: "iceberg",    label: "Iceberg / MinIO", service: "storage-iceberg + minio :9000", proves: "Event flushed to warm Parquet (s3://attest-warm)",          icon: Archive },
  { key: "mcp",        label: "MCP Gateway",     service: "attest-mcp-gateway :4242",      proves: "Attestation envelope queryable by external auditors",       icon: Shield },
];

type RunState = "idle" | "running" | "done" | "error";
type PollState = "idle" | "polling" | "finished";

const POLL_TIMEOUT_MS = 90_000;

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

function StageRow({
  meta,
  state,
  result,
  pendingLabel,
}: {
  meta: StageMeta;
  state: "pending" | "running" | "ok" | "fail";
  result?: StageResult;
  pendingLabel?: string;
}) {
  const Icon = meta.icon;
  return (
    <div className={cn(
      "flex items-start gap-3 rounded-md border px-3 py-2.5 transition-all",
      state === "ok"      && "border-emerald-500/30 bg-emerald-500/5",
      state === "fail"    && "border-yellow-500/30 bg-yellow-500/5",
      state === "running" && "border-primary/40 bg-primary/5 animate-pulse",
      state === "pending" && "border-border/40 bg-card/40",
    )}>
      <div className={cn(
        "mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md",
        state === "ok"      && "bg-emerald-500/20 text-emerald-400",
        state === "fail"    && "bg-yellow-500/20 text-yellow-400",
        state === "running" && "bg-primary/20 text-primary",
        state === "pending" && "bg-secondary text-muted-foreground",
      )}>
        {state === "ok"      ? <CheckCircle2 className="size-4" />
          : state === "fail"   ? <AlertTriangle className="size-4" />
          : state === "running" ? <Hourglass className="size-4 animate-spin" />
          : <Icon className="size-4" />}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-2">
          <span className="text-xs font-semibold">{meta.label}</span>
          <span className="font-mono text-[10px] text-muted-foreground">
            {state === "ok" || state === "fail"
              ? `${result?.latency_ms ?? 0} ms`
              : state === "running"
                ? "…"
                : meta.service.split(" ").pop()}
          </span>
        </div>
        <p className="mt-0.5 text-[10px] uppercase tracking-wide text-muted-foreground">{meta.service}</p>
        <p className="mt-1.5 text-xs leading-relaxed text-foreground/90">
          {state === "ok" && result   ? result.artifact
            : state === "fail" && result ? result.artifact
            : state === "running"        ? `Verifying — ${meta.proves}…`
            : pendingLabel              ? pendingLabel
            : meta.proves}
        </p>
      </div>
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────

export default function SimulatePage() {
  const [selected, setSelected] = useState<Scenario>(SCENARIOS[0]);
  const [params, setParams]     = useState<ScenarioParams>(SCENARIOS[0].defaultParams);
  const [runState, setRunState] = useState<RunState>("idle");

  const [hot, setHot]       = useState<SimulateResponse | null>(null);
  const [memory, setMemory] = useState<VerifyResponse | null>(null);
  const [pollState, setPollState] = useState<PollState>("idle");
  const [error, setError]   = useState<string | null>(null);

  const pollRef     = useRef<ReturnType<typeof setInterval> | null>(null);
  const pollStartRef = useRef<number>(0);

  // ── Stop polling on unmount ───────────────────────────────────────────
  useEffect(() => {
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, []);

  // ── Selection handler ────────────────────────────────────────────────
  const selectScenario = useCallback((s: Scenario) => {
    setSelected(s);
    setParams(s.defaultParams);
    setRunState("idle");
    setHot(null);
    setMemory(null);
    setPollState("idle");
    setError(null);
    if (pollRef.current) { clearInterval(pollRef.current); pollRef.current = null; }
  }, []);

  // ── Run simulation ───────────────────────────────────────────────────
  const runSimulation = useCallback(async () => {
    setRunState("running");
    setHot(null);
    setMemory(null);
    setPollState("idle");
    setError(null);
    if (pollRef.current) { clearInterval(pollRef.current); pollRef.current = null; }

    try {
      const res = await fetch("/api/simulate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ scenario_id: selected.id, params }),
      });
      const data: SimulateResponse = await res.json();

      setHot(data);
      setRunState(data.collector.ok && data.orchestrator.ok ? "done" : "error");

      // Begin polling memory section
      if (data.event_ids[0]) {
        pollStartRef.current = Date.now();
        setPollState("polling");
        const eventId  = data.event_ids[0];
        const username = data.username;
        const actionId = data.action_id;

        const stopPolling = () => {
          if (pollRef.current) { clearInterval(pollRef.current); pollRef.current = null; }
          setPollState("finished");
        };

        const poll = async () => {
          try {
            const r = await fetch(
              `/api/simulate/verify?event_id=${encodeURIComponent(eventId)}&username=${encodeURIComponent(username)}&action_id=${encodeURIComponent(actionId)}`,
              { cache: "no-store" },
            );
            const v: VerifyResponse = await r.json();
            setMemory(v);

            const allOk = v.risingwave.ok && v.detection.ok && v.iceberg.ok && v.mcp.ok;
            const elapsed = Date.now() - pollStartRef.current;
            if (allOk || elapsed > POLL_TIMEOUT_MS) stopPolling();
          } catch {/* keep polling */}
        };

        await poll();
        pollRef.current = setInterval(poll, 2_500);
      } else {
        setPollState("finished");
      }
    } catch (err) {
      setError(String(err));
      setRunState("error");
    }
  }, [selected, params]);

  const featureImpacts: FeatureImpact[] | null =
    hot?.classifier_evidence?.shap_values
      ? shapToFeatureImpacts(
          hot.classifier_evidence.shap_values,
          hot.classifier_evidence.input_features ?? {},
        )
      : null;

  // ── Stage-state mapper ────────────────────────────────────────────────
  const hotState = (key: StageKey): "pending" | "running" | "ok" | "fail" => {
    if (runState === "idle") return "pending";
    if (runState === "running" && !hot) return "running";
    const r = hot ? (hot[key as keyof SimulateResponse] as StageResult | undefined) : undefined;
    return r?.ok ? "ok" : r ? "fail" : "pending";
  };

  const memoryState = (key: "risingwave" | "detection" | "iceberg" | "mcp"): "pending" | "running" | "ok" | "fail" => {
    if (runState === "idle" || !hot) return "pending";
    if (!memory) return "running";
    const r = memory[key];
    if (r.ok) return "ok";
    // Polling still active → keep spinning. Polling finished → flip to "fail" so user knows it's not coming.
    return pollState === "polling" ? "running" : "fail";
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-0">
      {/* ── Header ── */}
      <header className="flex items-center gap-3 border-b border-border/80 px-6 py-4">
        <FlaskConical className="size-5 text-primary" />
        <div>
          <h1 className="text-sm font-semibold">Simulation Lab</h1>
          <p className="text-xs text-muted-foreground">Inject a realistic threat event and watch every layer of the platform participate</p>
        </div>
        {hot && (
          <div className="ml-auto flex items-center gap-2 rounded-md border border-primary/30 bg-primary/10 px-3 py-1.5">
            <Zap className="size-3.5 text-primary" />
            <span className="text-xs font-semibold">Hot path</span>
            <span className="font-mono text-xs text-primary">{hot.total_latency_ms} ms</span>
          </div>
        )}
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
              <ParamField label="Actor username" value={params.username} onChange={(v) => setParams((p) => ({ ...p, username: v }))} />
              <ParamField label="Source IP"      value={params.source_ip} onChange={(v) => setParams((p) => ({ ...p, source_ip: v }))} />
              <ParamField label="Region"         value={params.region}    onChange={(v) => setParams((p) => ({ ...p, region: v }))} />
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

        {/* ── Right: Pipeline + Results ── */}
        <div className="flex-1 overflow-y-auto p-5">
          {runState === "idle" && (
            <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
              <FlaskConical className="size-10 opacity-30" />
              <p className="text-sm">Select a scenario and click Run Simulation</p>
              <p className="max-w-md text-center text-xs leading-relaxed">
                The hot path runs in &lt; 5 seconds. The memory section verifies every parallel sink (RisingWave, Iceberg, MCP) and may take up to 60 s for downstream consumers to catch up.
              </p>
            </div>
          )}

          {runState === "error" && error && (
            <div className="flex items-start gap-3 rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-400">
              <AlertTriangle className="mt-0.5 size-4 shrink-0" />
              <div>
                <p className="font-medium">Simulation failed</p>
                <p className="mt-0.5 font-mono text-xs opacity-80">{error}</p>
              </div>
            </div>
          )}

          {(runState === "running" || runState === "done" || (runState === "error" && hot)) && (
            <div className="space-y-5">
              {/* ── HOT PATH ── */}
              <section>
                <div className="mb-2 flex items-baseline justify-between">
                  <div className="flex items-center gap-2">
                    <Zap className="size-3.5 text-primary" />
                    <h2 className="text-xs font-semibold uppercase tracking-[0.15em]">Hot Path</h2>
                    <span className="text-[10px] text-muted-foreground">decision latency</span>
                  </div>
                  {hot && (
                    <span className="font-mono text-xs text-primary">{hot.total_latency_ms} ms total</span>
                  )}
                </div>
                <div className="grid grid-cols-1 gap-2 lg:grid-cols-2">
                  {HOT_PATH_STAGES.map((meta) => (
                    <StageRow
                      key={meta.key}
                      meta={meta}
                      state={hotState(meta.key)}
                      result={hot ? (hot[meta.key as keyof SimulateResponse] as StageResult | undefined) : undefined}
                    />
                  ))}
                </div>
              </section>

              {/* ── Verdict + SHAP ── */}
              {hot && hot.orchestrator.ok && (
                <section className="grid grid-cols-1 gap-4 lg:grid-cols-2">
                  <div className="space-y-3">
                    {/* Verdict card */}
                    <div className="rounded-lg border border-border/80 bg-card/80 p-4">
                      <div className="flex items-start gap-3">
                        {hot.verdict === "benign" ? (
                          <CheckCircle2 className="size-5 text-emerald-400" />
                        ) : (
                          <ShieldAlert className="size-5 text-orange-400" />
                        )}
                        <div className="flex-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="text-sm font-semibold capitalize">{hot.verdict}</span>
                            <span className={cn("rounded border px-1.5 py-0.5 text-[10px] font-semibold uppercase", VERDICT_COLORS[hot.verdict] ?? VERDICT_COLORS.unknown)}>
                              {hot.verdict}
                            </span>
                            {hot.escalated && (
                              <span className="rounded border border-purple-500/30 bg-purple-500/15 px-1.5 py-0.5 text-[10px] font-semibold text-purple-400">
                                ESCALATED
                              </span>
                            )}
                          </div>
                          <p className="mt-1 font-mono text-[10px] text-muted-foreground">
                            {hot.execution_path} · action {hot.action_id.slice(0, 8)}…
                          </p>
                        </div>
                      </div>
                    </div>

                    <div className="rounded-lg border border-border/80 bg-card/80 p-4">
                      <h3 className="mb-3 text-xs font-semibold">Confidence</h3>
                      <div className="space-y-3">
                        <ConfidenceBar value={hot.calibrated_confidence} label="Calibrated confidence" />
                        <ConfidenceBar value={hot.novelty_score}         label="Novelty score (OOD)" />
                      </div>
                    </div>
                  </div>

                  {featureImpacts && featureImpacts.length > 0 && (
                    <ClassifierEvidencePanel features={featureImpacts} />
                  )}
                </section>
              )}

              {/* ── MEMORY ── */}
              <section>
                <div className="mb-2 flex items-baseline justify-between">
                  <div className="flex items-center gap-2">
                    <Layers className="size-3.5 text-muted-foreground" />
                    <h2 className="text-xs font-semibold uppercase tracking-[0.15em]">Memory</h2>
                    <span className="text-[10px] text-muted-foreground">parallel sinks · eventually consistent</span>
                  </div>
                  {memory && pollState === "polling" && (
                    <span className="flex items-center gap-1.5 font-mono text-xs text-muted-foreground">
                      <span className="size-2 animate-pulse rounded-full bg-primary" />
                      polling…
                    </span>
                  )}
                  {memory && pollState === "finished" && (() => {
                    const verified = [memory.risingwave, memory.detection, memory.iceberg, memory.mcp].filter((s) => s.ok).length;
                    return (
                      <span className="font-mono text-xs text-muted-foreground">
                        verified {verified}/4 · polling stopped
                      </span>
                    );
                  })()}
                </div>
                <div className="grid grid-cols-1 gap-2 lg:grid-cols-2">
                  {MEMORY_STAGES.map((meta) => (
                    <StageRow
                      key={meta.key}
                      meta={meta}
                      state={memoryState(meta.key as "risingwave" | "detection" | "iceberg" | "mcp")}
                      result={memory ? memory[meta.key as keyof VerifyResponse] : undefined}
                      pendingLabel={`Awaiting hot-path completion before verifying ${meta.label}…`}
                    />
                  ))}
                </div>
              </section>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Helpers ────────────────────────────────────────────────────────────────

function ParamField({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
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
