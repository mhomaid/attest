import { AlertQueue } from "@/components/workbench/alert-queue";
import { StatusBadge } from "@/components/workbench/status-badge";
import { alerts as mockAlerts } from "@/lib/mock-data";
import { parseEventsResponse } from "@/lib/ocsf-to-alert";

const CP_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";

async function fetchLiveAlerts() {
  try {
    const res = await fetch(`${CP_URL}/v1/events/recent`, {
      next: { revalidate: 5 },
    });
    if (!res.ok) return null;
    const data = await res.json();
    const alerts = parseEventsResponse(data);
    return alerts.length > 0 ? alerts : null;
  } catch {
    return null;
  }
}

export default async function QueuePage() {
  const liveAlerts = await fetchLiveAlerts();
  const alerts = liveAlerts ?? mockAlerts;
  const isLive = liveAlerts !== null;

  const queueStats = [
    { label: "Open alerts", value: String(alerts.filter((a) => a.state !== "auto-closed").length) },
    { label: "Awaiting review", value: String(alerts.filter((a) => a.state === "awaiting-review").length) },
    { label: "In-flight agents", value: String(alerts.filter((a) => a.state === "in-flight").length) },
    { label: "Auto-closed", value: String(alerts.filter((a) => a.state === "auto-closed").length) },
  ];

  return (
    <div className="space-y-3 p-3">
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="mb-2 flex items-center gap-2">
              <StatusBadge tone="info">Queue</StatusBadge>
              <StatusBadge tone={isLive ? "good" : "muted"}>
                {isLive ? "Live — control-plane connected" : "Mock data — control-plane offline"}
              </StatusBadge>
            </div>
            <h1 className="text-xl font-semibold tracking-tight">Alert Queue</h1>
            <p className="mt-1 max-w-3xl text-sm text-muted-foreground">
              Classifier-first triage surface with live agent context. Rows do not auto-scroll
              when new alerts land; the analyst keeps focus.
            </p>
          </div>

          <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
            {queueStats.map((stat) => (
              <div key={stat.label} className="rounded-md border border-border bg-background/60 px-3 py-2">
                <div className="metric-tabular font-mono text-lg font-semibold">{stat.value}</div>
                <div className="mt-1 text-[11px] text-muted-foreground">{stat.label}</div>
              </div>
            ))}
          </div>
        </div>
      </section>

      <AlertQueue alerts={alerts} />
    </div>
  );
}
