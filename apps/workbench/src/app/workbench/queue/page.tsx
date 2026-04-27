import { AlertQueueLive } from "@/components/workbench/alert-queue";
import { StatusBadge } from "@/components/workbench/status-badge";
import { alerts as mockAlerts } from "@/lib/mock-data";
import { parseFiredDetections } from "@/lib/detection-to-alert";
import type { Alert } from "@/lib/mock-data";

const API_URL = process.env.NEXT_PUBLIC_APP_URL
  ? `${process.env.NEXT_PUBLIC_APP_URL}/api/detections`
  : "http://localhost:3000/api/detections";

async function fetchLiveAlerts(): Promise<{ alerts: Alert[]; isLive: boolean }> {
  try {
    const res = await fetch(API_URL, {
      cache: "no-store",
    });
    if (!res.ok) return { alerts: mockAlerts, isLive: false };
    const data = await res.json();
    const alerts = parseFiredDetections(data);
    return alerts.length > 0
      ? { alerts, isLive: true }
      : { alerts: mockAlerts, isLive: false };
  } catch {
    return { alerts: mockAlerts, isLive: false };
  }
}

export default async function QueuePage() {
  const { alerts, isLive } = await fetchLiveAlerts();

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
              <StatusBadge tone={isLive ? "good" : "muted"}>
                {isLive
                  ? "Live — HELIQL detections connected"
                  : "Mock data — control-plane offline"}
              </StatusBadge>
            </div>
            <h1 className="text-xl font-semibold tracking-tight">Alert Queue</h1>
            <p className="mt-1 max-w-3xl text-sm text-muted-foreground">
              Real-time alerts fired by HELIQL detection rules. WebSocket pushes
              new matches as they arrive on the stream.
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
      </section>

      {/* AlertQueueLive is a client component that opens the WebSocket */}
      <AlertQueueLive initialAlerts={alerts} />
    </div>
  );
}
