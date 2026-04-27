import { ArrowUpRight, Clock3, ShieldAlert } from "lucide-react";
import Link from "next/link";
import { AgentLiveStatusPill } from "@/components/workbench/agent-live-status-pill";
import { StatusBadge } from "@/components/workbench/status-badge";
import type { Alert, AlertState, Severity } from "@/lib/mock-data";
import { cn } from "@/lib/utils";

const severityTone: Record<Severity, "critical" | "high" | "medium" | "low"> = {
  critical: "critical",
  high: "high",
  medium: "medium",
  low: "low",
};

const stateLabels: Record<AlertState, string> = {
  "in-flight": "In Flight",
  "awaiting-review": "Awaiting Review",
  "auto-closed": "Auto-Closed",
};

export function AlertQueue({ alerts }: { alerts: Alert[] }) {
  const grouped = Object.entries(stateLabels).map(([state, label]) => ({
    state: state as AlertState,
    label,
    alerts: alerts.filter((alert) => alert.state === state),
  }));

  return (
    <div className="space-y-3">
      {grouped.map((group) => (
        <section
          key={group.state}
          className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm"
        >
          <header className="sticky top-0 z-10 flex items-center justify-between border-b border-border bg-card/95 px-3 py-2 backdrop-blur">
            <div className="flex items-center gap-2">
              <ShieldAlert className="h-4 w-4 text-muted-foreground" />
              <h2 className="text-sm font-semibold">{group.label}</h2>
              <span className="rounded bg-secondary px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                {group.alerts.length}
              </span>
            </div>
            <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
              no auto-scroll
            </span>
          </header>

          <div className="divide-y divide-border/80">
            {group.alerts.length === 0 ? (
              <div className="px-3 py-6 text-sm text-muted-foreground">
                No alerts are currently in this state. New stream updates will appear without
                stealing focus.
              </div>
            ) : (
              group.alerts.map((alert) => <AlertRow key={alert.id} alert={alert} />)
            )}
          </div>
        </section>
      ))}
    </div>
  );
}

function AlertRow({ alert }: { alert: Alert }) {
  return (
    <Link
      href={`/workbench/cases/${alert.caseId}`}
      className={cn(
        "grid gap-2 px-3 py-2.5 transition-colors hover:bg-secondary/60 lg:grid-cols-[minmax(0,1.4fr)_9rem_8rem_13rem_6rem_2rem]",
        alert.state === "in-flight" && "bg-signal-live/5",
      )}
    >
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <StatusBadge tone={severityTone[alert.severity]}>{alert.severity}</StatusBadge>
          <span className="truncate text-sm font-medium">{alert.title}</span>
        </div>
        <p className="mt-1 line-clamp-2 max-w-3xl text-xs leading-5 text-muted-foreground">
          {alert.summary}
        </p>
      </div>

      <div className="text-xs">
        <div className="font-medium">{alert.source}</div>
        <div className="font-mono text-[11px] text-muted-foreground">{alert.region}</div>
      </div>

      <div className="min-w-0 text-xs">
        <div className="truncate font-medium">{alert.entity}</div>
        <div className="font-mono text-[11px] text-muted-foreground">{alert.technique}</div>
      </div>

      <AgentLiveStatusPill status={alert.agentStatus} />

      <div className="text-xs">
        <div className="font-mono text-sm text-foreground">{Math.round(alert.confidence * 100)}%</div>
        <div className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
          {alert.executionPath}
        </div>
      </div>

      <div className="flex items-center justify-between gap-2 text-muted-foreground lg:justify-end">
        <span className="inline-flex items-center gap-1 font-mono text-[10px]">
          <Clock3 className="h-3 w-3" />
          {alert.updatedAt}
        </span>
        <ArrowUpRight className="h-4 w-4" />
      </div>
    </Link>
  );
}
