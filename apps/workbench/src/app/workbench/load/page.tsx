"use client";

import { useEffect, useRef, useCallback, useState, useMemo, memo } from "react";
import {
  LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip,
  ResponsiveContainer,
} from "recharts";
import {
  Activity, Gauge, Play, Square, RefreshCw, Zap, AlertTriangle,
  Database, TrendingUp, Clock, Radio,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useLoadStore, type LoadConfig, type MetricsSnapshot } from "@/lib/stores/load-store";

// ── ENV ──────────────────────────────────────────────────────────────────────
// Browsers connect directly to control-plane WS (no Next.js proxy needed).
const _cpBase =
  process.env.NEXT_PUBLIC_CP_WS_URL ??
  (typeof window !== "undefined"
    ? `ws://${window.location.hostname}:8080`
    : "ws://localhost:8080");
const CP_WS_URL = _cpBase.replace(/\/$/, "") + "/v1/metrics/stream";

// ── Profile presets ────────────────────────────────────────────────────────

interface Preset {
  label: string;
  description: string;
  config: LoadConfig;
  color: string;
}

const PRESETS: Preset[] = [
  {
    label: "Smoke",
    description: "1k/sec · 30s",
    color: "text-emerald-400",
    config: { rate: 1_000, duration_secs: 30, scenario: "mixed", tenants: 2, seed_baselines: true, sampled_triage_pct: 5 },
  },
  {
    label: "Sustained",
    description: "10k/sec · 60s",
    color: "text-blue-400",
    config: { rate: 10_000, duration_secs: 60, scenario: "mixed", tenants: 3, seed_baselines: true, sampled_triage_pct: 5 },
  },
  {
    label: "Burst",
    description: "100k/sec · 60s",
    color: "text-orange-400",
    config: { rate: 100_000, duration_secs: 60, scenario: "mixed", tenants: 5, seed_baselines: true, sampled_triage_pct: 1 },
  },
  {
    label: "1M Challenge",
    description: "100k/sec · 120s",
    color: "text-red-400",
    config: { rate: 100_000, duration_secs: 120, scenario: "mixed", tenants: 10, seed_baselines: true, sampled_triage_pct: 0 },
  },
];

// ── Helpers ───────────────────────────────────────────────────────────────

function fmt(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

// ── Status badge ─────────────────────────────────────────────────────────────

const STATUS_STYLES: Record<string, string> = {
  idle:      "bg-secondary text-muted-foreground",
  starting:  "bg-blue-500/20 text-blue-400 animate-pulse",
  running:   "bg-emerald-500/20 text-emerald-400",
  stopping:  "bg-yellow-500/20 text-yellow-400",
  completed: "bg-primary/20 text-primary",
  error:     "bg-red-500/20 text-red-400",
};

const StatusBadge = memo(function StatusBadge({ status }: { status: string }) {
  return (
    <span className={cn(
      "rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
      STATUS_STYLES[status] ?? STATUS_STYLES.idle,
    )}>
      {status}
    </span>
  );
});

// ── Metric chart ─────────────────────────────────────────────────────────────

interface MetricChartProps {
  title: string;
  data: MetricsSnapshot[];
  dataKey: keyof MetricsSnapshot;
  strokeColor: string;
  icon: React.ReactNode;
  latest: number;
  unit?: string;
}

const MetricChart = memo(function MetricChart({
  title, data, dataKey, strokeColor, icon, latest, unit = "",
}: MetricChartProps) {
  const chartData = useMemo(
    () => data.map((s, i) => ({ t: i, v: s[dataKey] as number })),
    [data, dataKey],
  );

  return (
    <div className="rounded-lg border border-border/80 bg-card/80 p-3">
      <div className="mb-2 flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          {icon}
          <span className="text-xs font-medium">{title}</span>
        </div>
        <span className="font-mono text-sm font-semibold" style={{ color: strokeColor }}>
          {fmt(latest)}{unit}
        </span>
      </div>
      <ResponsiveContainer width="100%" height={90}>
        <LineChart data={chartData} margin={{ top: 2, right: 2, bottom: 2, left: 2 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.05)" />
          <XAxis dataKey="t" hide />
          <YAxis width={0} hide />
          <Tooltip
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            formatter={(v: any) => [`${fmt(Number(v ?? 0))}${unit}`, title]}
            contentStyle={{
              background: "hsl(var(--card))",
              border: "1px solid hsl(var(--border))",
              borderRadius: 6,
              fontSize: 11,
            }}
            labelStyle={{ display: "none" }}
          />
          <Line
            type="monotone"
            dataKey="v"
            stroke={strokeColor}
            strokeWidth={2}
            dot={false}
            isAnimationActive={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
});

// ── Config field ──────────────────────────────────────────────────────────────

const ConfigField = memo(function ConfigField({
  label, type = "text", value, onChange, disabled,
}: {
  label: string;
  type?: string;
  value: string;
  onChange: (v: string) => void;
  disabled?: boolean;
}) {
  return (
    <div>
      <label className="mb-1 block text-xs text-muted-foreground">{label}</label>
      <input
        type={type}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-md border border-border bg-card/60 px-2.5 py-1.5 font-mono text-xs text-foreground focus:border-primary/60 focus:outline-none disabled:opacity-50"
      />
    </div>
  );
});

// ── Total stat ────────────────────────────────────────────────────────────────

const TotalStat = memo(function TotalStat({
  label, value, icon,
}: {
  label: string;
  value: string;
  icon: React.ReactNode;
}) {
  return (
    <div>
      <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
        {icon}
        {label}
      </div>
      <p className="mt-1 font-mono text-lg font-semibold">{value}</p>
    </div>
  );
});

// ── Main page ─────────────────────────────────────────────────────────────────

export default function LoadPage() {
  // Fine-grained selectors — each slice re-renders only the relevant subtree.
  const status    = useLoadStore(s => s.status);
  const config    = useLoadStore(s => s.config);
  const history   = useLoadStore(s => s.history);
  const current   = useLoadStore(s => s.current);
  const errorMsg  = useLoadStore(s => s.errorMsg);
  const setConfig = useLoadStore(s => s.setConfig);
  const startRun  = useLoadStore(s => s.startRun);
  const stopRun   = useLoadStore(s => s.stopRun);
  const reset     = useLoadStore(s => s.reset);
  // Stable ref — Zustand actions never change identity across renders.
  const ingestSnapshot = useLoadStore(s => s.ingestSnapshot);

  const wsRef             = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const backoffRef        = useRef(1000);
  const mountedRef        = useRef(true);
  const connectWsRef      = useRef<() => void>(() => {});
  const [wsConnected, setWsConnected] = useState(false);

  // Track mount status so the WS close handler never schedules reconnects after unmount.
  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  // ── WebSocket with exponential back-off ───────────────────────────────────
  const connectWs = useCallback(() => {
    if (!mountedRef.current) return;
    if (wsRef.current && wsRef.current.readyState < 2) return; // already open/connecting

    try {
      const ws = new WebSocket(CP_WS_URL);
      wsRef.current = ws;

      ws.onopen = () => {
        if (!mountedRef.current) { ws.close(); return; }
        setWsConnected(true);
        backoffRef.current = 1000;
      };

      ws.onmessage = (e) => {
        try {
          const snap = JSON.parse(e.data as string) as MetricsSnapshot;
          ingestSnapshot(snap);
        } catch {/* ignore malformed frames */}
      };

      ws.onclose = () => {
        if (!mountedRef.current) return;
        setWsConnected(false);
        reconnectTimerRef.current = setTimeout(() => {
          backoffRef.current = Math.min(backoffRef.current * 1.5, 30_000);
          connectWsRef.current();
        }, backoffRef.current);
      };

      ws.onerror = () => {
        if (mountedRef.current) setWsConnected(false);
        ws.close();
      };
    } catch {/* WebSocket unavailable during SSR — safe to ignore */}
  }, [ingestSnapshot]);

  useEffect(() => {
    connectWsRef.current = connectWs;
  }, [connectWs]);

  useEffect(() => {
    connectWs();
    return () => {
      if (reconnectTimerRef.current) clearTimeout(reconnectTimerRef.current);
      wsRef.current?.close();
    };
  }, [connectWs]);

  // ── Derived ───────────────────────────────────────────────────────────────
  const elapsed = current ? fmt(current.total_events) + " events" : null;
  const busy = status === "starting" || status === "stopping";

  return (
    <div className="flex h-full min-h-0 flex-col gap-0">
      {/* ── Header ── */}
      <header className="flex items-center gap-3 border-b border-border/80 px-6 py-4">
        <Gauge className="size-5 text-primary" />
        <div>
          <h1 className="text-sm font-semibold">Load Lab</h1>
          <p className="text-xs text-muted-foreground">
            Direct-to-Kafka load generator · real-time metrics via WebSocket
          </p>
        </div>

        <div className="ml-auto flex items-center gap-3">
          {/* WS status indicator */}
          <div className={cn(
            "flex items-center gap-1.5 rounded-md border px-2 py-1 text-[10px]",
            wsConnected
              ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-400"
              : "border-border bg-secondary text-muted-foreground"
          )}>
            <span className={cn(
              "size-1.5 rounded-full",
              wsConnected ? "bg-emerald-400 animate-pulse" : "bg-muted-foreground",
            )} />
            {wsConnected ? "WS live" : "WS offline"}
          </div>

          <StatusBadge status={status} />

          {current && (
            <div className="flex items-center gap-1.5 rounded-md border border-primary/30 bg-primary/10 px-3 py-1.5">
              <Zap className="size-3.5 text-primary" />
              <span className="font-mono text-xs text-primary">{elapsed}</span>
            </div>
          )}

          {(status === "idle" || status === "completed" || status === "error") && (
            <button
              onClick={reset}
              className="flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-xs text-muted-foreground hover:border-border/80 hover:text-foreground"
            >
              <RefreshCw className="size-3" />
              Reset
            </button>
          )}

          <button
            disabled={busy}
            onClick={status === "running" ? stopRun : startRun}
            className={cn(
              "flex items-center gap-2 rounded-md px-4 py-1.5 text-xs font-semibold transition-all",
              status === "running"
                ? "border border-red-500/30 bg-red-500/20 text-red-400 hover:bg-red-500/30"
                : "bg-primary text-primary-foreground hover:bg-primary/90",
              busy && "cursor-not-allowed opacity-60",
            )}
          >
            {status === "running" ? (
              <><Square className="size-3" /> Stop</>
            ) : status === "starting" ? (
              <><span className="size-3 animate-spin rounded-full border-2 border-current border-t-transparent" /> Starting…</>
            ) : (
              <><Play className="size-3" /> Run</>
            )}
          </button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1 overflow-hidden">
        {/* ── Left: Config panel ── */}
        <aside className="w-64 shrink-0 overflow-y-auto border-r border-border/80 p-4 space-y-5">
          <div>
            <p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.15em] text-muted-foreground">Presets</p>
            <div className="space-y-1.5">
              {PRESETS.map((p) => (
                <button
                  key={p.label}
                  disabled={status === "running" || status === "starting"}
                  onClick={() => setConfig(p.config)}
                  className={cn(
                    "w-full rounded-md border px-3 py-2 text-left transition-colors",
                    config.rate === p.config.rate && config.duration_secs === p.config.duration_secs
                      ? "border-primary/40 bg-primary/10"
                      : "border-transparent bg-card/60 hover:border-border hover:bg-card",
                    (status === "running" || status === "starting") && "cursor-not-allowed opacity-50",
                  )}
                >
                  <div className="flex items-baseline justify-between">
                    <span className={cn("text-xs font-semibold", p.color)}>{p.label}</span>
                    <span className="text-[10px] text-muted-foreground">{p.description}</span>
                  </div>
                </button>
              ))}
            </div>
          </div>

          <div>
            <p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.15em] text-muted-foreground">Custom</p>
            <div className="space-y-3">
              <ConfigField
                label="Rate (events/sec)"
                type="number"
                value={String(config.rate)}
                onChange={(v) => setConfig({ rate: parseInt(v) || 1000 })}
                disabled={status === "running"}
              />
              <ConfigField
                label="Duration (sec)"
                type="number"
                value={String(config.duration_secs)}
                onChange={(v) => setConfig({ duration_secs: parseInt(v) || 30 })}
                disabled={status === "running"}
              />
              <ConfigField
                label="Tenants"
                type="number"
                value={String(config.tenants)}
                onChange={(v) => setConfig({ tenants: parseInt(v) || 1 })}
                disabled={status === "running"}
              />
              <div>
                <label className="mb-1 block text-xs text-muted-foreground">Scenario</label>
                <select
                  value={config.scenario}
                  disabled={status === "running"}
                  onChange={(e) => setConfig({ scenario: e.target.value as LoadConfig["scenario"] })}
                  className="w-full rounded-md border border-border bg-card/60 px-2 py-1.5 text-xs text-foreground focus:border-primary/60 focus:outline-none"
                >
                  <option value="mixed">Mixed (92% benign)</option>
                  <option value="attack">Attack only</option>
                  <option value="benign">Benign only</option>
                </select>
              </div>
              <label className="flex cursor-pointer items-center gap-2 text-xs text-muted-foreground">
                <input
                  type="checkbox"
                  checked={config.seed_baselines}
                  disabled={status === "running"}
                  onChange={(e) => setConfig({ seed_baselines: e.target.checked })}
                  className="accent-primary"
                />
                Seed baselines (pre-warms geo-anomaly detections)
              </label>
              <div>
                <label className="mb-1 flex items-center justify-between text-xs text-muted-foreground">
                  <span>Sampled triage</span>
                  <span className="font-mono text-foreground">
                    {config.sampled_triage_pct === 0 ? "off" : `${config.sampled_triage_pct}%`}
                  </span>
                </label>
                <input
                  type="range"
                  min={0} max={20} step={1}
                  value={config.sampled_triage_pct}
                  disabled={status === "running"}
                  onChange={(e) => setConfig({ sampled_triage_pct: parseInt(e.target.value) })}
                  className="w-full accent-purple-500"
                />
                <p className="mt-1 text-[10px] leading-relaxed text-muted-foreground">
                  Routes N% of events through the ML orchestrator — gives real triage p95 under load.
                </p>
              </div>
            </div>
          </div>
        </aside>

        {/* ── Right: Charts + totals ── */}
        <div className="flex-1 overflow-y-auto p-5 space-y-5">
          {errorMsg && (
            <div className="flex items-start gap-3 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-400">
              <AlertTriangle className="mt-0.5 size-4 shrink-0" />
              <div>
                <p className="font-semibold">Error</p>
                <p className="mt-0.5 font-mono opacity-80">{errorMsg}</p>
              </div>
            </div>
          )}

          {history.length === 0 && !errorMsg && (
            <div className="flex h-64 flex-col items-center justify-center gap-2 text-muted-foreground">
              <Gauge className="size-10 opacity-30" />
              <p className="text-sm">No data yet</p>
              <p className="max-w-xs text-center text-xs leading-relaxed">
                Start a run to see live throughput, consumer lag, detection rate, and triage latency charts.
              </p>
            </div>
          )}

          {history.length > 0 && (
            <>
              <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
                <MetricChart
                  title="Events / sec"
                  data={history}
                  dataKey="events_per_sec"
                  strokeColor="#3b82f6"
                  icon={<Radio className="size-3.5 text-blue-400" />}
                  latest={current?.events_per_sec ?? 0}
                />
                <MetricChart
                  title="Consumer lag"
                  data={history}
                  dataKey="consumer_lag"
                  strokeColor="#f59e0b"
                  icon={<Activity className="size-3.5 text-yellow-400" />}
                  latest={current?.consumer_lag ?? 0}
                  unit=" msg"
                />
                <MetricChart
                  title="Detections / sec"
                  data={history}
                  dataKey="detections_per_sec"
                  strokeColor="#f97316"
                  icon={<TrendingUp className="size-3.5 text-orange-400" />}
                  latest={current?.detections_per_sec ?? 0}
                />
                <MetricChart
                  title="Storage rows / sec"
                  data={history}
                  dataKey="clickhouse_rows_per_sec"
                  strokeColor="#a855f7"
                  icon={<Database className="size-3.5 text-purple-400" />}
                  latest={current?.clickhouse_rows_per_sec ?? 0}
                  unit=" rows"
                />
              </div>

              <div className="rounded-lg border border-border/80 bg-card/80 p-4">
                <p className="mb-3 text-[10px] font-semibold uppercase tracking-[0.15em] text-muted-foreground">
                  Session totals
                </p>
                <div className="grid grid-cols-2 gap-x-8 gap-y-3 lg:grid-cols-4">
                  <TotalStat
                    label="Events sent"
                    value={fmt(current?.total_events ?? 0)}
                    icon={<Database className="size-3.5 text-blue-400" />}
                  />
                  <TotalStat
                    label="Detections fired"
                    value={fmt(current?.total_detections ?? 0)}
                    icon={<Zap className="size-3.5 text-orange-400" />}
                  />
                  <TotalStat
                    label="Actual rate"
                    value={`${fmt(current?.current_load_gen_rate ?? 0)}/s`}
                    icon={<Activity className="size-3.5 text-emerald-400" />}
                  />
                  <TotalStat
                    label="Consumer lag"
                    value={`${fmt(current?.consumer_lag ?? 0)} msg`}
                    icon={<Clock className="size-3.5 text-yellow-400" />}
                  />
                </div>
                {/* Triage p95 — only shown when sampled triage is active */}
                {config.sampled_triage_pct > 0 && (current?.triage_p95_ms ?? 0) > 0 && (
                  <div className="mt-3 flex items-center gap-2 rounded-md border border-purple-500/30 bg-purple-500/10 px-3 py-2 text-xs">
                    <Clock className="size-3.5 shrink-0 text-purple-400" />
                    <span className="text-muted-foreground">ML triage p95 ({config.sampled_triage_pct}% sampled)</span>
                    <span className="ml-auto font-mono font-semibold text-purple-400">
                      {(current?.triage_p95_ms ?? 0).toFixed(1)} ms
                    </span>
                  </div>
                )}
                {config.sampled_triage_pct > 0 && (current?.triage_p95_ms ?? 0) === 0 && status === "running" && (
                  <div className="mt-3 flex items-center gap-2 rounded-md border border-border/40 bg-card/40 px-3 py-2 text-[11px] text-muted-foreground">
                    <Clock className="size-3.5 shrink-0" />
                    Triage p95 — accumulating samples ({config.sampled_triage_pct}% of events routed to orchestrator)…
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
