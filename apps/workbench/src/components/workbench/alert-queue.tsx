"use client";

import {
  type ColumnDef,
  type ColumnFiltersState,
  type SortingState,
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { ArrowUpDown, ArrowUpRight, ChevronDown, Clock3, RadioTower, WifiOff } from "lucide-react";
import Link from "next/link";
import { useMemo, useState } from "react";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useAlertsWs } from "@/hooks/use-alerts-ws";
import type { Alert, AlertState, Severity } from "@/lib/mock-data";
import { cn } from "@/lib/utils";

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
    header: () => (
      <span className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
        Triage
      </span>
    ),
    cell: ({ row }) => <AgentLiveStatusPill status={row.original.agentStatus} />,
  },
  {
    accessorKey: "confidence",
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
    cell: ({ row }) => (
      <span className="inline-flex items-center gap-1 font-mono text-[10px] text-muted-foreground">
        <Clock3 className="h-3 w-3" />
        {row.getValue<string>("updatedAt")}
      </span>
    ),
  },
  {
    id: "actions",
    cell: ({ row }) => (
      <Link
        href={`/workbench/cases/${row.original.caseId}`}
        className="grid h-7 w-7 place-items-center rounded-md border border-border text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
      >
        <ArrowUpRight className="h-3.5 w-3.5" />
      </Link>
    ),
    enableSorting: false,
    enableHiding: false,
  },
];

// ── Group header ──────────────────────────────────────────────────────────────

const stateLabels: Record<AlertState, string> = {
  "in-flight":       "In Flight",
  "awaiting-review": "Awaiting Review",
  "auto-closed":     "Auto-Closed",
};

// ── Live wrapper ──────────────────────────────────────────────────────────────

export function AlertQueueLive({ initialAlerts }: { initialAlerts: Alert[] }) {
  const { newAlerts, wsStatus } = useAlertsWs();

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

export function AlertQueue({ alerts }: { alerts: Alert[] }) {
  const [activeState, setActiveState] = useState<AlertState>("awaiting-review");

  const filteredAlerts = alerts.filter((a) => a.state === activeState);

  const counts: Record<AlertState, number> = {
    "in-flight":       alerts.filter((a) => a.state === "in-flight").length,
    "awaiting-review": alerts.filter((a) => a.state === "awaiting-review").length,
    "auto-closed":     alerts.filter((a) => a.state === "auto-closed").length,
  };

  return (
    <div className="space-y-2">
      {/* State tabs */}
      <div className="flex gap-1 rounded-lg border border-border bg-card/80 p-1">
        {(Object.entries(stateLabels) as [AlertState, string][]).map(([state, label]) => (
          <button
            key={state}
            onClick={() => setActiveState(state)}
            className={cn(
              "flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
              activeState === state
                ? "bg-secondary text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {label}
            <Badge variant="secondary" className="font-mono text-[10px] px-1.5 h-4">
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

function AlertDataTable({ alerts }: { alerts: Alert[] }) {
  const [sorting, setSorting]               = useState<SortingState>([]);
  const [columnFilters, setColumnFilters]   = useState<ColumnFiltersState>([]);
  const [columnVisibility, setColumnVisibility] = useState({});
  const [globalFilter, setGlobalFilter]     = useState("");

  const table = useReactTable({
    data: alerts,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    onColumnVisibilityChange: setColumnVisibility,
    onGlobalFilterChange: setGlobalFilter,
    globalFilterFn: "includesString",
    state: { sorting, columnFilters, columnVisibility, globalFilter },
    initialState: { pagination: { pageSize: 15 } },
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

      {/* Table */}
      <Table>
        <TableHeader>
          {table.getHeaderGroups().map((hg) => (
            <TableRow key={hg.id} className="border-border/80 hover:bg-transparent">
              {hg.headers.map((header) => (
                <TableHead key={header.id} className="h-8 px-3 bg-card/95">
                  {header.isPlaceholder
                    ? null
                    : flexRender(header.column.columnDef.header, header.getContext())}
                </TableHead>
              ))}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody>
          {table.getRowModel().rows.length ? (
            table.getRowModel().rows.map((row) => (
              <TableRow
                key={row.id}
                className={cn(
                  "border-border/80 transition-colors hover:bg-secondary/50",
                  row.original.state === "in-flight" && "bg-signal-live/5",
                )}
              >
                {row.getVisibleCells().map((cell) => (
                  <TableCell key={cell.id} className="px-3 py-2.5">
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </TableCell>
                ))}
              </TableRow>
            ))
          ) : (
            <TableRow>
              <TableCell colSpan={columns.length} className="py-8 text-center text-sm text-muted-foreground">
                No alerts in this state.
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>

      {/* Pagination */}
      <div className="flex items-center justify-between border-t border-border px-3 py-2">
        <span className="font-mono text-[11px] text-muted-foreground">
          {table.getFilteredRowModel().rows.length} row(s) total
        </span>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5">
            <span className="text-xs text-muted-foreground">Rows per page</span>
            <select
              value={table.getState().pagination.pageSize}
              onChange={(e) => table.setPageSize(Number(e.target.value))}
              className="h-7 rounded-md border border-border bg-background px-2 text-xs text-foreground"
            >
              {[10, 15, 25, 50].map((size) => (
                <option key={size} value={size}>{size}</option>
              ))}
            </select>
          </div>
          <span className="font-mono text-[11px] text-muted-foreground">
            Page {table.getState().pagination.pageIndex + 1} of {Math.max(1, table.getPageCount())}
          </span>
          <div className="flex gap-1">
            <Button variant="outline" size="sm" className="h-7 w-7 p-0 border-border/70" onClick={() => table.setPageIndex(0)} disabled={!table.getCanPreviousPage()}>«</Button>
            <Button variant="outline" size="sm" className="h-7 w-7 p-0 border-border/70" onClick={() => table.previousPage()} disabled={!table.getCanPreviousPage()}>‹</Button>
            <Button variant="outline" size="sm" className="h-7 w-7 p-0 border-border/70" onClick={() => table.nextPage()} disabled={!table.getCanNextPage()}>›</Button>
            <Button variant="outline" size="sm" className="h-7 w-7 p-0 border-border/70" onClick={() => table.setPageIndex(table.getPageCount() - 1)} disabled={!table.getCanNextPage()}>»</Button>
          </div>
        </div>
      </div>
    </div>
  );
}
