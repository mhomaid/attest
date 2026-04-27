"use client";

import { ArrowUpRight, Clock3, RadioTower, ShieldAlert, WifiOff } from "lucide-react";
import Link from "next/link";
import { useMemo, useState } from "react";
import { AgentLiveStatusPill } from "@/components/workbench/agent-live-status-pill";
import { StatusBadge } from "@/components/workbench/status-badge";
import { useAlertsWs } from "@/hooks/use-alerts-ws";
import type { Alert, AlertState, Severity } from "@/lib/mock-data";
import { cn } from "@/lib/utils";

const severityTone: Record<Severity, "critical" | "high" | "medium" | "low"> = {
  critical: "critical",
  high:     "high",
  medium:   "medium",
  low:      "low",
};

const stateLabels: Record<AlertState, string> = {
  "in-flight":       "In Flight",
  "awaiting-review": "Awaiting Review",
  "auto-closed":     "Auto-Closed",
};

// ── Live wrapper (client component) ──────────────────────────────────────────

/**
 * AlertQueueLive mounts the WebSocket hook and merges real-time arrivals
 * (prepended) with the server-rendered initial snapshot.
 */
export function AlertQueueLive({ initialAlerts }: { initialAlerts: Alert[] }) {
  const { newAlerts, wsStatus } = useAlertsWs();

  // Deduplicate by id so a server-rendered alert that also arrives over WS
  // isn't shown twice.
  const alerts = useMemo(() => {
    const seen = new Set<string>();
    const merged = [...newAlerts, ...initialAlerts];
    return merged.filter((a) => {
      if (seen.has(a.id)) return false;
      seen.add(a.id);
      return true;
    });
  }, [newAlerts, initialAlerts]);

  return (
    <div className="space-y-1">
      {/* WebSocket status bar */}
      <div className="flex items-center gap-2 rounded-md border border-border/50 bg-card/60 px-3 py-1.5">
        {wsStatus === "connected" ? (
          <>
            <span className="relative flex h-2 w-2">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-signal-good opacity-75" />
              <span className="relative inline-flex h-2 w-2 rounded-full bg-signal-good" />
            </span>
            <RadioTower className="h-3.5 w-3.5 text-signal-good" />
            <span className="font-mono text-[11px] text-muted-foreground">
              Live — WebSocket connected
            </span>
          </>
        ) : (
          <>
            <WifiOff className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="font-mono text-[11px] text-muted-foreground">
              {wsStatus === "connecting" ? "Connecting…" : "Reconnecting…"}
            </span>
          </>
        )}
        <span className="ml-auto font-mono text-[10px] text-muted-foreground">
          {alerts.length} alert{alerts.length !== 1 ? "s" : ""}
        </span>
      </div>

      <AlertQueue alerts={alerts} />
    </div>
  );
}

// ── Pure display component ────────────────────────────────────────────────────

export function AlertQueue({ alerts }: { alerts: Alert[] }) {
  const grouped = Object.entries(stateLabels).map(([state, label]) => ({
    state:  state as AlertState,
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
                No alerts in this state. New stream updates appear without stealing focus.
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

// ── Single alert row ──────────────────────────────────────────────────────────

function AlertRow({ alert }: { alert: Alert }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div
      className={cn(
        "transition-colors hover:bg-secondary/60",
        alert.state === "in-flight" && "bg-signal-live/5",
      )}
    >
      <Link
        href={`/workbench/cases/${alert.caseId}`}
        className="grid gap-2 px-3 py-2.5 lg:grid-cols-[minmax(0,1.4fr)_9rem_8rem_13rem_6rem_2rem]"
        onClick={(e) => {
          // On small viewports expand the row instead of navigating.
          if (window.innerWidth < 1024) {
            e.preventDefault();
            setExpanded((v) => !v);
          }
        }}
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
          <div className="font-mono text-sm text-foreground">
            {Math.round(alert.confidence * 100)}%
          </div>
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

      {/* Expanded detail on small screens */}
      {expanded && (
        <div className="border-t border-border/60 bg-secondary/30 px-3 py-2 text-xs text-muted-foreground">
          <p className="font-medium text-foreground">{alert.summary}</p>
          <p className="mt-1 font-mono text-[10px]">event: {alert.id}</p>
          <p className="font-mono text-[10px]">technique: {alert.technique}</p>
        </div>
      )}
    </div>
  );
}
