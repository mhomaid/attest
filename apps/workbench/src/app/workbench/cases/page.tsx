import { ArrowUpRight, ShieldAlert } from "lucide-react";
import Link from "next/link";
import { StatusBadge } from "@/components/workbench/status-badge";
import { parseFiredDetections } from "@/lib/detection-to-alert";
import type { Alert } from "@/lib/mock-data";

const API_URL = process.env.NEXT_PUBLIC_APP_URL
  ? `${process.env.NEXT_PUBLIC_APP_URL}/api/detections`
  : "http://localhost:3000/api/detections";

async function fetchCases(): Promise<{ cases: Alert[]; isLive: boolean; offline: boolean }> {
  try {
    const res = await fetch(API_URL, { cache: "no-store" });
    if (!res.ok) return { cases: [], isLive: false, offline: true };
    const data = await res.json();
    const alerts = parseFiredDetections(data);
    return { cases: alerts, isLive: alerts.length > 0, offline: false };
  } catch {
    return { cases: [], isLive: false, offline: true };
  }
}

const severityTone = {
  critical: "critical",
  high:     "high",
  medium:   "medium",
  low:      "low",
} as const;

export default async function CasesPage() {
  const { cases, isLive, offline } = await fetchCases();

  const open   = cases.filter((c) => c.state !== "auto-closed");
  const closed = cases.filter((c) => c.state === "auto-closed");

  return (
    <div className="space-y-3 p-3">
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="mb-2 flex items-center gap-2">
              <StatusBadge tone="info">Cases</StatusBadge>
              <StatusBadge tone={offline ? "muted" : isLive ? "good" : "info"}>
                {offline ? "Offline — control-plane unreachable" : isLive ? "Live" : "Connected — no detections yet"}
              </StatusBadge>
            </div>
            <h1 className="text-xl font-semibold tracking-tight">All Cases</h1>
            <p className="mt-1 text-sm text-muted-foreground">
              Every alert that has been triaged or is currently under investigation.
            </p>
          </div>

          <div className="grid grid-cols-3 gap-2">
            {(["in-flight", "awaiting-review", "auto-closed"] as const).map((state) => (
              <div
                key={state}
                className="rounded-md border border-border bg-background/60 px-3 py-2"
              >
                <div className="font-mono text-lg font-semibold">
                  {cases.filter((c) => c.state === state).length}
                </div>
                <div className="mt-1 text-[11px] capitalize text-muted-foreground">
                  {state.replace("-", " ")}
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {[
        { label: "Open", items: open },
        { label: "Auto-Closed", items: closed },
      ].map(({ label, items }) => (
        <section
          key={label}
          className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm"
        >
          <header className="flex items-center gap-2 border-b border-border bg-card/95 px-3 py-2">
            <ShieldAlert className="h-4 w-4 text-muted-foreground" />
            <h2 className="text-sm font-semibold">{label}</h2>
            <span className="rounded bg-secondary px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
              {items.length}
            </span>
          </header>

          <div className="divide-y divide-border/80">
            {items.length === 0 ? (
              <p className="px-3 py-6 text-sm text-muted-foreground">
                {offline
                  ? "Control-plane is unreachable. Start the backend and reload."
                  : "No cases in this state. Run a load test or send events to generate detections."}
              </p>
            ) : (
              items.map((c) => (
                <Link
                  key={c.id}
                  href={`/workbench/cases/${c.caseId}`}
                  className="grid gap-2 px-3 py-2.5 transition-colors hover:bg-secondary/50 lg:grid-cols-[minmax(0,1.4fr)_8rem_8rem_6rem_2rem]"
                >
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <StatusBadge tone={severityTone[c.severity]}>{c.severity}</StatusBadge>
                      <span className="truncate text-sm font-medium">{c.title}</span>
                    </div>
                    <p className="mt-1 line-clamp-1 text-xs text-muted-foreground">{c.summary}</p>
                  </div>
                  <div className="text-xs">
                    <div className="truncate font-medium">{c.entity}</div>
                    <div className="font-mono text-[11px] text-muted-foreground">{c.region}</div>
                  </div>
                  <div className="text-xs">
                    <div className="font-medium">{c.source}</div>
                    <div className="font-mono text-[11px] text-muted-foreground">{c.technique}</div>
                  </div>
                  <div className="text-xs">
                    <div className="font-mono font-semibold">
                      {Math.round(c.confidence * 100)}%
                    </div>
                    <div className="font-mono text-[10px] uppercase tracking-wide text-muted-foreground">
                      {c.executionPath}
                    </div>
                  </div>
                  <div className="flex items-center justify-end text-muted-foreground">
                    <ArrowUpRight className="h-4 w-4" />
                  </div>
                </Link>
              ))
            )}
          </div>
        </section>
      ))}
    </div>
  );
}
