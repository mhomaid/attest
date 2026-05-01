import { ShieldAlert, ShieldCheck } from "lucide-react";
import { AlertsTable } from "@/components/workbench/alert-queue";
import { StatusBadge } from "@/components/workbench/status-badge";
import { parseFiredDetections } from "@/lib/detection-to-alert";
import { enrichAlertsWithPersisted } from "@/lib/server/enrich-alert";
import type { Alert } from "@/lib/mock-data";

export const dynamic = "force-dynamic";

const CP_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";

type LiveStatus = "live" | "connected" | "offline";

async function fetchCases(): Promise<{
  cases: Alert[];
  status: LiveStatus;
}> {
  try {
    const res = await fetch(`${CP_URL}/v1/detections/fired`, {
      cache: "no-store",
      signal: AbortSignal.timeout(1500),
    });
    if (!res.ok) return { cases: [], status: "offline" };
    const data = await res.json();
    const parsed = parseFiredDetections(data);
    if (parsed.length === 0) return { cases: [], status: "connected" };
    const enriched = await enrichAlertsWithPersisted(parsed);
    return { cases: enriched, status: "live" };
  } catch {
    return { cases: [], status: "offline" };
  }
}

export default async function CasesPage() {
  const { cases, status } = await fetchCases();

  const open = cases.filter((c) => c.state !== "auto-closed");
  const closed = cases.filter((c) => c.state === "auto-closed");

  const stateCounts = (["in-flight", "awaiting-review", "auto-closed"] as const)
    .map((state) => ({
      state,
      count: cases.filter((c) => c.state === state).length,
    }));

  const sections: Array<{
    key: string;
    label: string;
    items: Alert[];
    icon: typeof ShieldAlert;
    tone: "info" | "muted";
  }> = [
    { key: "open", label: "Open", items: open, icon: ShieldAlert, tone: "info" },
    {
      key: "closed",
      label: "Auto-Closed",
      items: closed,
      icon: ShieldCheck,
      tone: "muted",
    },
  ];

  return (
    <div className="space-y-3 p-3">
      {/* Header card */}
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="mb-2 flex items-center gap-2">
              <StatusBadge tone="info">Cases</StatusBadge>
              <StatusBadge
                tone={
                  status === "live"
                    ? "good"
                    : status === "connected"
                      ? "info"
                      : "muted"
                }
              >
                {status === "live"
                  ? "Live — HELIQL detections connected"
                  : status === "connected"
                    ? "Connected — no detections yet"
                    : "Offline — control-plane unreachable"}
              </StatusBadge>
            </div>
            <h1 className="text-xl font-semibold tracking-tight">All Cases</h1>
            <p className="mt-1 max-w-3xl text-sm text-muted-foreground">
              Every alert that has been triaged or is currently under
              investigation. Use the search and column filters in each table to
              drill down.
            </p>
          </div>

          <div className="grid grid-cols-3 gap-2">
            {stateCounts.map(({ state, count }) => (
              <div
                key={state}
                className="rounded-md border border-border bg-background/60 px-3 py-2"
              >
                <div className="metric-tabular font-mono text-lg font-semibold">
                  {count}
                </div>
                <div className="mt-1 text-[11px] capitalize text-muted-foreground">
                  {state.replace("-", " ")}
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Open + Auto-Closed sections — both use the same AlertsTable */}
      {sections.map(({ key, label, items, icon: Icon, tone }) => (
        <section key={key} className="space-y-2">
          <header className="flex items-center gap-2 px-1">
            <Icon className="h-4 w-4 text-muted-foreground" />
            <h2 className="text-sm font-semibold">{label}</h2>
            <StatusBadge tone={tone}>{items.length}</StatusBadge>
          </header>

          {items.length === 0 ? (
            <div className="rounded-lg border border-dashed border-border bg-card/40 px-3 py-6 text-sm text-muted-foreground">
              {status === "offline"
                ? "Control-plane is unreachable. Start the backend and reload."
                : `No cases in this state. ${
                    label === "Open"
                      ? "Run a load test or send events to generate detections."
                      : "Cases auto-close once the calibrated confidence is below the auto-close threshold."
                  }`}
            </div>
          ) : (
            <AlertsTable alerts={items} />
          )}
        </section>
      ))}
    </div>
  );
}
