"use client";

import {
  type ColumnDef,
  type ColumnFiltersState,
  type SortingState,
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowUpDown, ArrowUpRight, ChevronDown, Clock3, RadioTower, WifiOff } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useRef, useState, useCallback } from "react";
import { AgentLiveStatusPill } from "@/components/workbench/agent-live-status-pill";
import { StatusBadge } from "@/components/workbench/status-badge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { useAlertsWs } from "@/hooks/use-alerts-ws";
import type { Alert, Severity } from "@/lib/mock-data";
import { cn } from "@/lib/utils";

// ── Relative time ─────────────────────────────────────────────────────────────

function relativeTime(iso: string): string {
  try {
    const date = new Date(iso.replace(" ", "T"));
    const s = Math.max(0, Math.floor((Date.now() - date.getTime()) / 1000));
    if (s < 60)  return `${s}s ago`;
    const m = Math.floor(s / 60);
    if (m < 60)  return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 24)  return `${h}h ago`;
    return `${Math.floor(h / 24)}d ago`;
  } catch { return iso; }
}

/** Ticks every 30 s so the column stays current without full re-renders. */
function RelativeTime({ iso }: { iso: string }) {
  const [label, setLabel] = useState(() => relativeTime(iso));
  const tick = useCallback(() => setLabel(relativeTime(iso)), [iso]);
  useEffect(() => {
    const id = setInterval(tick, 30_000);
    return () => clearInterval(id);
  }, [tick]);
  return (
    <span className="inline-flex items-center gap-1 font-mono text-[10px] text-muted-foreground">
      <Clock3 className="h-3 w-3" />
      {label}
    </span>
  );
}

const severityTone: Record<Severity, "critical" | "high" | "medium" | "low"> = {
  critical: "critical",
  high: "high",
  medium: "medium",
  low: "low",
};

const severityOrder: Record<Severity, number> = {
  critical: 0, high: 1, medium: 2, low: 3,
};

// ── Column definitions ────────────────────────────────────────────────────────

const columns: ColumnDef<Alert>[] = [
  {
    accessorKey: "severity",
    size: 100,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-3 h-8 text-xs font-medium uppercase tracking-wider text-muted-foreground"
        onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
      >
        Severity
        <ArrowUpDown className="ml-1.5 h-3 w-3" />
      </Button>
    ),
    cell: ({ row }) => (
      <StatusBadge tone={severityTone[row.getValue<Severity>("severity")]}>
        {row.getValue<string>("severity")}
      </StatusBadge>
    ),
    sortingFn: (a, b) =>
      severityOrder[a.original.severity] - severityOrder[b.original.severity],
    filterFn: (row, _id, value) =>
      row.original.severity.toLowerCase().includes((value as string).toLowerCase()),
  },
  {
    accessorKey: "title",
    size: 280,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-3 h-8 text-xs font-medium uppercase tracking-wider text-muted-foreground"
        onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
      >
        Rule / Summary
        <ArrowUpDown className="ml-1.5 h-3 w-3" />
      </Button>
    ),
    cell: ({ row }) => (
      <div className="min-w-0">
        <div className="truncate text-sm font-medium">{row.getValue<string>("title")}</div>
        <p className="mt-0.5 line-clamp-1 text-xs text-muted-foreground">{row.original.summary}</p>
      </div>
    ),
  },
  {
    accessorKey: "source",
    size: 120,
    header: () => (
      <span className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
        Source
      </span>
    ),
    cell: ({ row }) => (
      <div className="text-xs">
        <div className="font-medium">{row.getValue<string>("source")}</div>
        <div className="font-mono text-[11px] text-muted-foreground">{row.original.region}</div>
      </div>
    ),
  },
  {
    accessorKey: "entity",
    size: 130,
    header: () => (
      <span className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
        Entity
      </span>
    ),
    cell: ({ row }) => (
      <div className="min-w-0 text-xs">
        <div className="truncate font-medium">{row.getValue<string>("entity")}</div>
        <div className="font-mono text-[11px] text-muted-foreground">{row.original.technique}</div>
      </div>
    ),
  },
  {
    accessorKey: "agentStatus",
    size: 110,
    header: () => (
      <span className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
        Triage
      </span>
    ),
    cell: ({ row }) => <AgentLiveStatusPill status={row.original.agentStatus} />,
  },
  {
    accessorKey: "confidence",
    size: 110,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-3 h-8 text-xs font-medium uppercase tracking-wider text-muted-foreground"
        onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
      >
        Confidence
        <ArrowUpDown className="ml-1.5 h-3 w-3" />
      </Button>
    ),
    cell: ({ row }) => (
      <div className="text-xs">
        <div className="font-mono text-sm text-foreground">
          {Math.round(row.original.confidence * 100)}%
        </div>
        <div className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
          {row.original.executionPath}
        </div>
      </div>
    ),
  },
  {
    accessorKey: "updatedAt",
    size: 110,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-3 h-8 text-xs font-medium uppercase tracking-wider text-muted-foreground"
        onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
      >
        Time
        <ArrowUpDown className="ml-1.5 h-3 w-3" />
      </Button>
    ),
    cell: ({ row }) => <RelativeTime iso={row.getValue<string>("updatedAt")} />,
  },
  {
    id: "actions",
    size: 48,
    cell: ({ row }) => (
      <Link
        href={`/workbench/cases/${row.original.caseId}`}
        onClick={(e) => e.stopPropagation()}
        className="grid h-7 w-7 place-items-center rounded-md border border-border text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
        aria-label={`Open case ${row.original.caseId}`}
      >
        <ArrowUpRight className="h-3.5 w-3.5" />
      </Link>
    ),
    enableSorting: false,
    enableHiding: false,
  },
];

// ── Live wrapper ──────────────────────────────────────────────────────────────

export function AlertQueueLive({ initialAlerts }: { initialAlerts: Alert[] }) {
  const { newAlerts, wsStatus } = useAlertsWs();
  const router = useRouter();

  // Refresh the server component data every time this view mounts (navigating back)
  // so the alert list is always current, not served from the router cache.
  useEffect(() => {
    router.refresh();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

// ── Main queue with tabs per state ────────────────────────────────────────────

type QueueTabState = "in-flight" | "awaiting-review";

const tabLabels: Record<QueueTabState, string> = {
  "in-flight": "In Flight",
  "awaiting-review": "Awaiting Review",
};

export function AlertQueue({ alerts }: { alerts: Alert[] }) {
  const [activeState, setActiveState] = useState<QueueTabState>("awaiting-review");
  const [autoGroupOpen, setAutoGroupOpen] = useState(false);

  const autoClosedAlerts = alerts.filter((a) => a.state === "auto-closed");
  const tabAlerts = alerts.filter((a) => a.state !== "auto-closed");
  const filteredAlerts = tabAlerts.filter((a) => a.state === activeState);

  const counts: Record<QueueTabState, number> = {
    "in-flight": tabAlerts.filter((a) => a.state === "in-flight").length,
    "awaiting-review": tabAlerts.filter((a) => a.state === "awaiting-review").length,
  };

  return (
    <div className="space-y-2">
      {autoClosedAlerts.length > 0 ? (
        <div className="sticky top-0 z-10 rounded-lg border border-border bg-card/95 shadow-sm backdrop-blur">
          <button
            type="button"
            onClick={() => setAutoGroupOpen(!autoGroupOpen)}
            className="flex w-full items-center justify-between px-3 py-2 text-left text-xs font-medium"
          >
            <span>Auto-closed ({autoClosedAlerts.length})</span>
            <span className="text-muted-foreground">{autoGroupOpen ? "▾" : "▸"}</span>
          </button>
          {autoGroupOpen ? (
            <div className="border-t border-border p-2">
              <AlertDataTable alerts={autoClosedAlerts} compact />
            </div>
          ) : null}
        </div>
      ) : null}

      <div className="flex gap-1 rounded-lg border border-border bg-card/80 p-1">
        {(Object.entries(tabLabels) as [QueueTabState, string][]).map(([state, label]) => (
          <button
            key={state}
            type="button"
            onClick={() => setActiveState(state)}
            className={cn(
              "flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
              activeState === state
                ? "bg-secondary text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {label}
            <Badge variant="secondary" className="h-4 px-1.5 font-mono text-[10px]">
              {counts[state]}
            </Badge>
          </button>
        ))}
      </div>

      <AlertDataTable alerts={filteredAlerts} />
    </div>
  );
}

// ── DataTable ─────────────────────────────────────────────────────────────────

function AlertDataTable({ alerts, compact = false }: { alerts: Alert[]; compact?: boolean }) {
  const router = useRouter();
  const [sorting, setSorting]               = useState<SortingState>([]);
  const [columnFilters, setColumnFilters]   = useState<ColumnFiltersState>([]);
  const [columnVisibility, setColumnVisibility] = useState({});
  const [globalFilter, setGlobalFilter]     = useState("");
  const scrollParentRef                     = useRef<HTMLDivElement>(null);

  const table = useReactTable({
    data: alerts,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    onColumnVisibilityChange: setColumnVisibility,
    onGlobalFilterChange: setGlobalFilter,
    globalFilterFn: "includesString",
    state: { sorting, columnFilters, columnVisibility, globalFilter },
    defaultColumn: { minSize: 48, maxSize: 800 },
  });

  const rows = table.getRowModel().rows;
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollParentRef.current,
    estimateSize: () => (compact ? 44 : 54),
    overscan: 12,
  });

  return (
    <div className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm">
      {/* Toolbar */}
      <div className="flex flex-wrap items-center gap-2 border-b border-border px-3 py-2">
        <Input
          placeholder="Search alerts…"
          value={globalFilter}
          onChange={(e) => setGlobalFilter(e.target.value)}
          className="h-7 max-w-xs text-xs bg-background/60 border-border/70"
        />
        <Input
          placeholder="Filter by entity…"
          value={(table.getColumn("entity")?.getFilterValue() as string) ?? ""}
          onChange={(e) => table.getColumn("entity")?.setFilterValue(e.target.value)}
          className="h-7 max-w-[160px] text-xs bg-background/60 border-border/70"
        />
        <Input
          placeholder="Filter by severity…"
          value={(table.getColumn("severity")?.getFilterValue() as string) ?? ""}
          onChange={(e) => table.getColumn("severity")?.setFilterValue(e.target.value)}
          className="h-7 max-w-[140px] text-xs bg-background/60 border-border/70"
        />
        <div className="ml-auto flex items-center gap-2">
          <span className="font-mono text-[11px] text-muted-foreground">
            {table.getFilteredRowModel().rows.length} rows
          </span>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" size="sm" className="h-7 text-xs gap-1 border-border/70">
                Columns <ChevronDown className="h-3 w-3" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="text-xs">
              {table.getAllColumns().filter((c) => c.getCanHide()).map((col) => (
                <DropdownMenuCheckboxItem
                  key={col.id}
                  className="capitalize text-xs"
                  checked={col.getIsVisible()}
                  onCheckedChange={(v) => col.toggleVisibility(!!v)}
                >
                  {col.id}
                </DropdownMenuCheckboxItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      {/* Virtualized body — scrolls all filtered rows (5k+) at stable cost */}
      <div ref={scrollParentRef} className="max-h-[min(70vh,720px)] overflow-auto">
        <table className="w-full caption-bottom text-sm">
          <thead className="sticky top-0 z-[1] border-b border-border bg-card shadow-sm [&_tr]:border-b">
            {table.getHeaderGroups().map((hg) => (
              <tr key={hg.id} className="border-border/80 hover:bg-transparent">
                {hg.headers.map((header) => (
                  <th
                    key={header.id}
                    className="h-8 px-3 text-left align-middle text-xs font-medium uppercase tracking-wider text-muted-foreground"
                    style={{ width: header.getSize() }}
                  >
                    {header.isPlaceholder
                      ? null
                      : flexRender(header.column.columnDef.header, header.getContext())}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody
            className="relative [&_tr:last-child]:border-0"
            style={rows.length ? { height: `${rowVirtualizer.getTotalSize()}px` } : undefined}
          >
            {rows.length ? (
              rowVirtualizer.getVirtualItems().map((vi) => {
                const row = rows[vi.index]!;
                const caseHref = `/workbench/cases/${row.original.caseId}`;
                return (
                  <tr
                    key={row.id}
                    data-index={vi.index}
                    data-testid="alert-row"
                    data-execution-path={row.original.executionPath}
                    ref={rowVirtualizer.measureElement}
                    role="link"
                    tabIndex={0}
                    className={cn(
                      "absolute left-0 w-full cursor-pointer border-b border-border/80 transition-colors hover:bg-secondary/50",
                      row.original.state === "in-flight" && "bg-signal-live/5",
                    )}
                    style={{
                      transform: `translateY(${vi.start}px)`,
                    }}
                    onMouseEnter={() => router.prefetch(caseHref)}
                    onClick={() => router.push(caseHref)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        router.push(caseHref);
                      }
                    }}
                  >
                    {row.getVisibleCells().map((cell) => (
                      <td
                        key={cell.id}
                        className="px-3 py-2.5 align-middle"
                        style={{ width: cell.column.getSize() }}
                      >
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </td>
                    ))}
                  </tr>
                );
              })
            ) : (
              <tr>
                <td
                  colSpan={columns.length}
                  className="py-8 text-center text-sm text-muted-foreground"
                >
                  No alerts in this state.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="flex items-center border-t border-border px-3 py-2">
        <span className="font-mono text-[11px] text-muted-foreground">
          {table.getFilteredRowModel().rows.length} row(s) — virtual scroll
        </span>
      </div>
    </div>
  );
}
