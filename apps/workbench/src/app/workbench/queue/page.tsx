import { Activity } from "lucide-react";
import { AlertQueueLive } from "@/components/workbench/alert-queue";
import { StatusBadge } from "@/components/workbench/status-badge";
import { alerts as mockAlerts } from "@/lib/mock-data";
import { parseFiredDetections } from "@/lib/detection-to-alert";
import type { Alert } from "@/lib/mock-data";

const CP_URL     = process.env.CONTROL_PLANE_URL  ?? "http://localhost:8080";
const ARROYO_URL = process.env.ARROYO_URL         ?? "http://localhost:5115";

type LiveStatus = "live" | "connected" | "offline";

async function fetchLiveAlerts(): Promise<{ alerts: Alert[]; status: LiveStatus }> {
  try {
    const res = await fetch(`${CP_URL}/v1/detections/fired`, {
      cache: "no-store",
      signal: AbortSignal.timeout(4000),
    });
    if (!res.ok) return { alerts: mockAlerts, status: "offline" };
    const data = await res.json();
    const alerts = parseFiredDetections(data);
    return alerts.length > 0
      ? { alerts, status: "live" }
      : { alerts: mockAlerts, status: "connected" };
  } catch {
    return { alerts: mockAlerts, status: "offline" };
  }
}

async function fetchArroyoPipelines(): Promise<{ name: string; state: string }[]> {
  try {
    const res = await fetch(`${ARROYO_URL}/api/v1/pipelines`, {
      cache: "no-store",
      signal: AbortSignal.timeout(3000),
    });
    if (!res.ok) return [];
    const body = await res.json();
    return (body.data ?? []).map((p: { name: string }) => ({ name: p.name, state: "Running" }));
  } catch {
    return [];
  }
}

export default async function QueuePage() {
  const [{ alerts, status }, arroyoPipelines] = await Promise.all([
    fetchLiveAlerts(),
    fetchArroyoPipelines(),
  ]);

  const queueStats = [
    {
      label: "Open alerts",
      value: String(alerts.filter((a) => a.state !== "auto-closed").length),
    },
    {
      label: "Awaiting review",
      value: String(alerts.filter((a) => a.state === "awaiting-review").length),
    },
    {
      label: "In-flight agents",
      value: String(alerts.filter((a) => a.state === "in-flight").length),
    },
    {
      label: "Auto-closed",
      value: String(alerts.filter((a) => a.state === "auto-closed").length),
    },
  ];

  return (
    <div className="space-y-3 p-3">
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="mb-2 flex items-center gap-2">
              <StatusBadge tone="info">Queue</StatusBadge>
              <StatusBadge
                tone={status === "live" ? "good" : status === "connected" ? "info" : "muted"}
              >
                {status === "live"
                  ? "Live — HELIQL detections connected"
                  : status === "connected"
                  ? "Connected — no detections yet"
                  : "Mock data — control-plane unreachable"}
              </StatusBadge>
              {arroyoPipelines.length > 0 && (
                <StatusBadge tone="good">
                  Arroyo {arroyoPipelines.length} pipeline{arroyoPipelines.length !== 1 ? "s" : ""} running
                </StatusBadge>
              )}
            </div>
            <h1 className="text-xl font-semibold tracking-tight">Alert Queue</h1>
            <p className="mt-1 max-w-3xl text-sm text-muted-foreground">
              Real-time alerts fired by HELIQL detection rules. WebSocket pushes new matches as
              they arrive on the stream. Arroyo CEP alerts are routed via the{" "}
              <code className="rounded bg-secondary px-1 py-0.5 font-mono text-[11px]">alerts</code>{" "}
              Kafka topic.
            </p>
          </div>

          <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
            {queueStats.map((stat) => (
              <div
                key={stat.label}
                className="rounded-md border border-border bg-background/60 px-3 py-2"
              >
                <div className="metric-tabular font-mono text-lg font-semibold">
                  {stat.value}
                </div>
                <div className="mt-1 text-[11px] text-muted-foreground">{stat.label}</div>
              </div>
            ))}
          </div>
        </div>

        {/* Arroyo pipeline chips */}
        {arroyoPipelines.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-2 border-t border-border pt-3">
            {arroyoPipelines.map((p) => (
              <div
                key={p.name}
                className="inline-flex items-center gap-1.5 rounded-md border border-signal-good/30 bg-signal-good/10 px-2 py-1 text-[11px]"
              >
                <Activity className="h-3 w-3 text-signal-good" />
                <code className="font-mono text-signal-good">{p.name}</code>
                <span className="text-muted-foreground">{p.state}</span>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* AlertQueueLive is a client component that opens the WebSocket */}
      <AlertQueueLive initialAlerts={alerts} />
    </div>
  );
}
