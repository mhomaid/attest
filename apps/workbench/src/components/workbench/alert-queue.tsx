"use client";

import {
  type ColumnDef,
  type ColumnFiltersState,
  type FilterFn,
  type SortingState,
  type VisibilityState,
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import {
  ArrowUpDown,
  ArrowUpRight,
  Bot,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  Clock3,
  Loader2,
  RadioTower,
  ShieldCheck,
  WifiOff,
} from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useAlertsWs } from "@/hooks/use-alerts-ws";
import type { Alert, Severity } from "@/lib/mock-data";
import { cn } from "@/lib/utils";

// ── Helpers ───────────────────────────────────────────────────────────────────

function relativeTime(iso: string): string {
  try {
    const date = new Date(iso.replace(" ", "T"));
    const s = Math.max(0, Math.floor((Date.now() - date.getTime()) / 1000));
    if (s < 60) return `${s}s ago`;
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}h ago`;
    return `${Math.floor(h / 24)}d ago`;
  } catch {
    return iso;
  }
}

function formatAbsoluteTime(iso: string): string {
  try {
    const date = new Date(iso.replace(" ", "T"));
    if (Number.isNaN(date.getTime())) return iso;
    return date.toLocaleString(undefined, {
      month: "short",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

function RelativeTime({ iso }: { iso: string }) {
  const [label, setLabel] = useState(() => relativeTime(iso));
  const [mounted, setMounted] = useState(false);
  const tick = useCallback(() => setLabel(relativeTime(iso)), [iso]);
  useEffect(() => {
    setMounted(true);
    tick();
    const id = setInterval(tick, 30_000);
    return () => clearInterval(id);
  }, [tick]);
  return (
    <div className="font-mono text-xs">
      <div className="inline-flex items-center gap-1 text-muted-foreground">
        <Clock3 className="h-3 w-3 shrink-0" />
        <span suppressHydrationWarning>{mounted ? label : ""}</span>
      </div>
      <div className="text-[10px] text-muted-foreground/60" title={iso}>
        {formatAbsoluteTime(iso)}
      </div>
    </div>
  );
}

const severityTone: Record<Severity, "critical" | "high" | "medium" | "low"> =
  {
    critical: "critical",
    high: "high",
    medium: "medium",
    low: "low",
  };

const severityOrder: Record<Severity, number> = {
  critical: 0,
  high: 1,
  medium: 2,
  low: 3,
};

function formatVerdict(verdict: Alert["verdict"]): string {
  return verdict.replace(/_/g, " ");
}

function formatPath(path: Alert["executionPath"]): string {
  if (path === "llm") return "LLM";
  if (path === "hybrid") return "Hybrid";
  return "Classifier";
}

function statusTone(status: Alert["agentStatus"]["status"]) {
  if (status === "complete") return "text-signal-good";
  if (status === "running") return "text-signal-live";
  if (status === "degraded") return "text-severity-medium";
  return "text-muted-foreground";
}

function CompactTriageCell({ alert }: { alert: Alert }) {
  const status = alert.agentStatus.status;
  const Icon =
    status === "complete" ? CheckCircle2 : status === "running" ? Loader2 : Bot;
  const label =
    status === "complete"
      ? "Complete"
      : status === "running"
        ? "Running"
        : status === "degraded"
          ? "Degraded"
          : "Not triaged";
  return (
    <div className="text-xs">
      <div
        className={cn(
          "flex items-center gap-1.5 font-semibold",
          statusTone(status),
        )}
      >
        <Icon
          className={cn(
            "h-3.5 w-3.5 shrink-0",
            status === "running" && "animate-spin",
          )}
        />
        <span>{label}</span>
      </div>
      <div
        className="mt-0.5 line-clamp-1 text-[11px] text-muted-foreground"
        title={alert.agentStatus.step}
      >
        {alert.agentStatus.step}
      </div>
      <div className="mt-0.5 inline-flex items-center gap-1 font-mono text-[10px] uppercase tracking-[0.12em] text-muted-foreground">
        <ShieldCheck className="h-3 w-3 shrink-0" />
        {formatVerdict(alert.verdict)}
      </div>
    </div>
  );
}

// ── Global filter — searches all meaningful fields ────────────────────────────

const alertGlobalFilter: FilterFn<Alert> = (row, _columnId, filterValue) => {
  const q = String(filterValue).toLowerCase();
  if (!q) return true;
  const a = row.original;
  return [
    a.id,
    a.caseId,
    a.title,
    a.summary,
    a.source,
    a.entity,
    a.region,
    a.severity,
    a.technique,
    a.verdict,
    a.executionPath,
    a.agentStatus.step,
    a.agentStatus.status,
    a.agentStatus.agent,
    String(Math.round(a.confidence * 100)),
  ].some((v) => v?.toLowerCase().includes(q));
};

// ── Column definitions ────────────────────────────────────────────────────────

const columns: ColumnDef<Alert>[] = [
  // ─ Event ID (leftmost data column) ──────────────────────────────────────────
  {
    accessorKey: "id",
    size: 120,
    header: () => (
      <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
        Event ID
      </span>
    ),
    cell: ({ row }) => (
      <span
        className="block truncate font-mono text-[11px] text-foreground"
        title={row.original.id}
      >
        {row.original.id.slice(0, 12)}
      </span>
    ),
    filterFn: (row, _id, value) =>
      row.original.id.toLowerCase().includes((value as string).toLowerCase()),
  },

  // ─ Severity ─────────────────────────────────────────────────────────────────
  {
    accessorKey: "severity",
    size: 96,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-2 h-7 px-2 text-[11px] font-medium uppercase tracking-wider text-muted-foreground"
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
      row.original.severity
        .toLowerCase()
        .includes((value as string).toLowerCase()),
  },

  // ─ Rule / Summary ────────────────────────────────────────────────────────────
  {
    accessorKey: "title",
    size: 320,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-2 h-7 px-2 text-[11px] font-medium uppercase tracking-wider text-muted-foreground"
        onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
      >
        Rule / Summary
        <ArrowUpDown className="ml-1.5 h-3 w-3" />
      </Button>
    ),
    cell: ({ row }) => (
      <div className="min-w-0">
        <div className="truncate text-sm font-semibold">
          {row.getValue<string>("title")}
        </div>
        <p
          className="line-clamp-1 text-xs text-muted-foreground"
          title={row.original.summary}
        >
          {row.original.summary || "No summary available"}
        </p>
        {row.original.technique ? (
          <Badge variant="outline" className="mt-1 h-4 px-1.5 text-[10px]">
            {row.original.technique}
          </Badge>
        ) : null}
      </div>
    ),
    filterFn: (row, _id, value) => {
      const q = (value as string).toLowerCase();
      return (
        row.original.title.toLowerCase().includes(q) ||
        (row.original.summary?.toLowerCase().includes(q) ?? false) ||
        (row.original.technique?.toLowerCase().includes(q) ?? false)
      );
    },
  },

  // ─ Source ────────────────────────────────────────────────────────────────────
  {
    accessorKey: "source",
    size: 150,
    header: () => (
      <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
        Source
      </span>
    ),
    cell: ({ row }) => (
      <div className="min-w-0 text-xs">
        <div className="truncate font-medium" title={row.original.source}>
          {row.getValue<string>("source") || "unknown"}
        </div>
        <div
          className="truncate font-mono text-[11px] text-muted-foreground"
          title={row.original.region}
        >
          {row.original.region || "unknown region"}
        </div>
      </div>
    ),
    filterFn: (row, _id, value) => {
      const q = (value as string).toLowerCase();
      return (
        row.original.source.toLowerCase().includes(q) ||
        row.original.region.toLowerCase().includes(q)
      );
    },
  },

  // ─ Entity ────────────────────────────────────────────────────────────────────
  {
    accessorKey: "entity",
    size: 180,
    header: () => (
      <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
        Entity
      </span>
    ),
    cell: ({ row }) => (
      <div className="min-w-0 text-xs">
        <div className="truncate font-medium" title={row.original.entity}>
          {row.getValue<string>("entity") || "unknown"}
        </div>
        <div className="truncate font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
          {row.original.technique || formatPath(row.original.executionPath)}
        </div>
      </div>
    ),
    filterFn: (row, _id, value) =>
      row.original.entity
        .toLowerCase()
        .includes((value as string).toLowerCase()),
  },

  // ─ Triage ────────────────────────────────────────────────────────────────────
  {
    accessorKey: "agentStatus",
    size: 170,
    header: () => (
      <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
        Triage
      </span>
    ),
    cell: ({ row }) => <CompactTriageCell alert={row.original} />,
    filterFn: (row, _id, value) => {
      const q = (value as string).toLowerCase();
      const s = row.original.agentStatus;
      return (
        s.status.toLowerCase().includes(q) ||
        s.step.toLowerCase().includes(q) ||
        row.original.verdict.toLowerCase().includes(q)
      );
    },
  },

  // ─ Confidence ────────────────────────────────────────────────────────────────
  {
    accessorKey: "confidence",
    size: 140,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-2 h-7 px-2 text-[11px] font-medium uppercase tracking-wider text-muted-foreground"
        onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
      >
        Confidence
        <ArrowUpDown className="ml-1.5 h-3 w-3" />
      </Button>
    ),
    cell: ({ row }) => (
      <div className="text-xs">
        <div className="flex items-center justify-between gap-2">
          <span className="font-mono text-sm font-semibold text-foreground">
            {Math.round(row.original.confidence * 100)}%
          </span>
          <span className="font-mono text-[10px] uppercase tracking-[0.16em] text-muted-foreground">
            {formatPath(row.original.executionPath)}
          </span>
        </div>
        <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-secondary">
          <div
            className="h-full rounded-full bg-primary"
            style={{
              width: `${Math.max(3, Math.round(row.original.confidence * 100))}%`,
            }}
          />
        </div>
      </div>
    ),
    filterFn: (row, _id, value) =>
      String(Math.round(row.original.confidence * 100)).includes(
        value as string,
      ),
  },

  // ─ Time ──────────────────────────────────────────────────────────────────────
  {
    accessorKey: "updatedAt",
    size: 130,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-2 h-7 px-2 text-[11px] font-medium uppercase tracking-wider text-muted-foreground"
        onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
      >
        Time
        <ArrowUpDown className="ml-1.5 h-3 w-3" />
      </Button>
    ),
    cell: ({ row }) => <RelativeTime iso={row.getValue<string>("updatedAt")} />,
    filterFn: (row, _id, value) => {
      const q = (value as string).toLowerCase();
      return (
        row.original.updatedAt.toLowerCase().includes(q) ||
        formatAbsoluteTime(row.original.updatedAt).toLowerCase().includes(q)
      );
    },
  },

  // ─ Actions ───────────────────────────────────────────────────────────────────
  {
    id: "actions",
    size: 56,
    enableSorting: false,
    enableHiding: false,
    cell: ({ row }) => (
      <div className="flex justify-end">
        <Link
          href={`/workbench/cases/${row.original.caseId}`}
          onClick={(e) => e.stopPropagation()}
          className="grid h-7 w-7 place-items-center rounded-md border border-border text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
          aria-label={`Open case ${row.original.caseId}`}
        >
          <ArrowUpRight className="h-3.5 w-3.5" />
        </Link>
      </div>
    ),
  },
];

// ── Live wrapper ──────────────────────────────────────────────────────────────

export function AlertQueueLive({ initialAlerts }: { initialAlerts: Alert[] }) {
  const { newAlerts, wsStatus } = useAlertsWs();

  // Note: we used to call `router.refresh()` on mount, but that re-runs the
  // entire server component (control-plane fetch + N×DB lookup + Arroyo) on
  // every navigation back to the queue, which made navigation feel like it
  // hung. The initial server data is fresh enough; the WebSocket pushes new
  // alerts in real time, so a refetch on mount is pure waste.
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

// ── Tabs per state ────────────────────────────────────────────────────────────

type QueueTabState = "in-flight" | "awaiting-review";

const tabLabels: Record<QueueTabState, string> = {
  "in-flight": "In Flight",
  "awaiting-review": "Awaiting Review",
};

export function AlertQueue({ alerts }: { alerts: Alert[] }) {
  const [activeState, setActiveState] =
    useState<QueueTabState>("awaiting-review");
  const [autoGroupOpen, setAutoGroupOpen] = useState(false);

  const autoClosedAlerts = alerts.filter((a) => a.state === "auto-closed");
  const tabAlerts = alerts.filter((a) => a.state !== "auto-closed");
  const filteredAlerts = tabAlerts.filter((a) => a.state === activeState);

  const counts: Record<QueueTabState, number> = {
    "in-flight": tabAlerts.filter((a) => a.state === "in-flight").length,
    "awaiting-review": tabAlerts.filter((a) => a.state === "awaiting-review")
      .length,
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
            <span className="text-muted-foreground">
              {autoGroupOpen ? "▾" : "▸"}
            </span>
          </button>
          {autoGroupOpen ? (
            <div className="border-t border-border p-2">
              <AlertDataTable alerts={autoClosedAlerts} />
            </div>
          ) : null}
        </div>
      ) : null}

      <div className="flex gap-1 rounded-lg border border-border bg-card/80 p-1">
        {(Object.entries(tabLabels) as [QueueTabState, string][]).map(
          ([state, label]) => (
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
              <Badge
                variant="secondary"
                className="h-4 px-1.5 font-mono text-[10px]"
              >
                {counts[state]}
              </Badge>
            </button>
          ),
        )}
      </div>

      <AlertDataTable alerts={filteredAlerts} />
    </div>
  );
}

// ── Standard shadcn DataTable (no custom <table>, no virtualization) ─────────

/**
 * Public AlertsTable: full-featured shadcn data table for `Alert[]`.
 *
 * Used by both the Queue page (one table per tab) and the Cases page
 * (one table per Open / Auto-Closed section). All sorting, filtering,
 * column visibility, and pagination behavior is centralized here so
 * both pages stay visually identical.
 */
export function AlertsTable({ alerts }: { alerts: Alert[] }) {
  return <AlertDataTable alerts={alerts} />;
}

function AlertDataTable({ alerts }: { alerts: Alert[] }) {
  const router = useRouter();
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([]);
  const [columnVisibility, setColumnVisibility] = useState<VisibilityState>({});
  const [globalFilter, setGlobalFilter] = useState("");
  const [pagination, setPagination] = useState({ pageIndex: 0, pageSize: 25 });

  const table = useReactTable({
    data: alerts,
    columns,
    state: {
      sorting,
      columnFilters,
      columnVisibility,
      globalFilter,
      pagination,
    },
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    onColumnVisibilityChange: setColumnVisibility,
    onGlobalFilterChange: setGlobalFilter,
    onPaginationChange: setPagination,
    globalFilterFn: alertGlobalFilter,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    defaultColumn: { minSize: 40, maxSize: 800 },
  });

  const filteredCount = table.getFilteredRowModel().rows.length;
  const pageRows = table.getRowModel().rows;

  return (
    <div className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm">
      {/* Toolbar */}
      <div className="flex flex-wrap items-center gap-2 border-b border-border px-3 py-2">
        <Input
          placeholder="Search all columns…"
          value={globalFilter}
          onChange={(e) => setGlobalFilter(e.target.value)}
          className="h-7 max-w-xs text-xs bg-background/60 border-border/70"
        />
        <Input
          placeholder="Filter by entity…"
          value={(table.getColumn("entity")?.getFilterValue() as string) ?? ""}
          onChange={(e) =>
            table.getColumn("entity")?.setFilterValue(e.target.value)
          }
          className="h-7 max-w-[148px] text-xs bg-background/60 border-border/70"
        />
        <Input
          placeholder="Filter by source…"
          value={(table.getColumn("source")?.getFilterValue() as string) ?? ""}
          onChange={(e) =>
            table.getColumn("source")?.setFilterValue(e.target.value)
          }
          className="h-7 max-w-[136px] text-xs bg-background/60 border-border/70"
        />
        <Input
          placeholder="Filter by severity…"
          value={
            (table.getColumn("severity")?.getFilterValue() as string) ?? ""
          }
          onChange={(e) =>
            table.getColumn("severity")?.setFilterValue(e.target.value)
          }
          className="h-7 max-w-[136px] text-xs bg-background/60 border-border/70"
        />
        <div className="ml-auto flex items-center gap-2">
          <span className="font-mono text-[11px] text-muted-foreground">
            {filteredCount} row{filteredCount !== 1 ? "s" : ""}
          </span>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                className="h-7 gap-1 border-border/70 text-xs"
              >
                Columns <ChevronDown className="h-3 w-3" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="text-xs">
              {table
                .getAllColumns()
                .filter((c) => c.getCanHide())
                .map((col) => (
                  <DropdownMenuCheckboxItem
                    key={col.id}
                    className="text-xs capitalize"
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

      {/* Table — uses shadcn primitives, no custom table layout */}
      <Table>
        <TableHeader className="sticky top-0 z-[1] bg-card">
          {table.getHeaderGroups().map((hg) => (
            <TableRow key={hg.id} className="hover:bg-transparent">
              {hg.headers.map((header) => (
                <TableHead
                  key={header.id}
                  style={{ width: header.getSize() }}
                  className="h-9 px-3 align-middle text-[11px] font-medium uppercase tracking-wider"
                >
                  {header.isPlaceholder
                    ? null
                    : flexRender(
                        header.column.columnDef.header,
                        header.getContext(),
                      )}
                </TableHead>
              ))}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody>
          {pageRows.length ? (
            pageRows.map((row) => {
              const caseHref = `/workbench/cases/${row.original.caseId}`;
              return (
                <TableRow
                  key={row.id}
                  data-testid="alert-row"
                  data-execution-path={row.original.executionPath}
                  role="link"
                  tabIndex={0}
                  className={cn(
                    "cursor-pointer",
                    row.original.state === "in-flight" && "bg-signal-live/5",
                  )}
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
                    <TableCell
                      key={cell.id}
                      style={{ width: cell.column.getSize() }}
                      className="px-3 py-3 align-top"
                    >
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext(),
                      )}
                    </TableCell>
                  ))}
                </TableRow>
              );
            })
          ) : (
            <TableRow>
              <TableCell
                colSpan={columns.length}
                className="h-24 text-center text-sm text-muted-foreground"
              >
                No alerts in this state.
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>

      {/* Pagination footer */}
      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border px-3 py-2">
        <span className="font-mono text-[11px] text-muted-foreground">
          Page {table.getState().pagination.pageIndex + 1} of{" "}
          {table.getPageCount() || 1}
          {" · "}
          {filteredCount} row{filteredCount !== 1 ? "s" : ""}
        </span>
        <div className="flex items-center gap-1">
          <Button
            variant="outline"
            size="sm"
            className="hidden h-7 w-7 p-0 lg:inline-flex"
            onClick={() => table.setPageIndex(0)}
            disabled={!table.getCanPreviousPage()}
            aria-label="First page"
          >
            <ChevronsLeft className="h-3.5 w-3.5" />
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7 w-7 p-0"
            onClick={() => table.previousPage()}
            disabled={!table.getCanPreviousPage()}
            aria-label="Previous page"
          >
            <ChevronLeft className="h-3.5 w-3.5" />
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7 w-7 p-0"
            onClick={() => table.nextPage()}
            disabled={!table.getCanNextPage()}
            aria-label="Next page"
          >
            <ChevronRight className="h-3.5 w-3.5" />
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="hidden h-7 w-7 p-0 lg:inline-flex"
            onClick={() => table.setPageIndex(table.getPageCount() - 1)}
            disabled={!table.getCanNextPage()}
            aria-label="Last page"
          >
            <ChevronsRight className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>
    </div>
  );
}
