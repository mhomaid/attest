"use client";

import {
  type ColumnDef,
  type ColumnFiltersState,
  type FilterFn,
  type SortingState,
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import {
  ArrowUpDown,
  BrainIcon,
  CpuIcon,
  FileWarning,
  GavelIcon,
  Loader2,
  RefreshCw,
  SearchIcon,
  ShieldIcon,
  WrenchIcon,
} from "lucide-react";
import type { ReactNode } from "react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Badge } from "@/components/reui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { KafkaTraceStep, WsTraceStatus } from "@/hooks/use-case-trace-ws";

// ── Types ─────────────────────────────────────────────────────────────────────

type HttpTraceStep = {
  kind: string;
  agent_action_id: string;
  agent_id: string;
  execution_path: string;
  verdict: string;
  tool_call_count: number;
  belief_count: number;
};

type TraceResponse = { steps?: HttpTraceStep[]; error?: string };

type TraceRow = {
  id: string;
  kind: string;
  summary: string;
  agent_id: string;
  execution_path: string;
  tool_call_count: number;
  belief_count: number;
  agent_action_id: string;
};

// ── Helpers ───────────────────────────────────────────────────────────────────

async function fetchCaseTrace(caseId: string): Promise<TraceResponse> {
  const res = await fetch(`/api/cases/${encodeURIComponent(caseId)}/trace`);
  return (await res.json()) as TraceResponse;
}

function kafkaToDisplay(s: KafkaTraceStep): HttpTraceStep {
  return {
    kind: s.step_kind,
    agent_action_id: s.agent_action_id,
    agent_id: s.agent_id,
    execution_path: s.execution_path,
    verdict: s.summary,
    tool_call_count: 0,
    belief_count: 0,
  };
}

function toTraceRow(s: HttpTraceStep, index: number): TraceRow {
  return {
    id: `${s.agent_action_id}-${s.kind}-${index}`,
    kind: s.kind,
    summary: s.verdict,
    agent_id: s.agent_id,
    execution_path: s.execution_path,
    tool_call_count: s.tool_call_count,
    belief_count: s.belief_count,
    agent_action_id: s.agent_action_id,
  };
}

function kindIcon(kind: string): ReactNode {
  switch (kind) {
    case "intermediate_belief":
      return <BrainIcon className="h-3.5 w-3.5" />;
    case "tool_call":
      return <WrenchIcon className="h-3.5 w-3.5" />;
    case "final_verdict":
      return <GavelIcon className="h-3.5 w-3.5" />;
    case "envelope":
      return <ShieldIcon className="h-3.5 w-3.5" />;
    case "classifier_complete":
      return <CpuIcon className="h-3.5 w-3.5" />;
    default:
      return <SearchIcon className="h-3.5 w-3.5" />;
  }
}

function kindVariant(
  kind: string,
): React.ComponentProps<typeof Badge>["variant"] {
  if (kind === "tool_call") return "primary-light";
  if (kind === "final_verdict" || kind === "triage_complete")
    return "success-light";
  return "secondary";
}

function prettyKind(kind: string): string {
  return kind.replaceAll("_", " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

function formatPath(path: string): string {
  if (path === "llm") return "LLM";
  if (path === "hybrid") return "Hybrid";
  if (path === "classifier") return "Classifier";
  return path;
}

// ── Global filter ─────────────────────────────────────────────────────────────

const traceGlobalFilter: FilterFn<TraceRow> = (row, _col, filterValue) => {
  const q = String(filterValue).toLowerCase();
  if (!q) return true;
  const r = row.original;
  return [
    r.kind,
    r.summary,
    r.agent_id,
    r.execution_path,
    r.agent_action_id,
    String(r.tool_call_count),
    String(r.belief_count),
  ].some((v) => v?.toLowerCase().includes(q));
};

// ── Column definitions ────────────────────────────────────────────────────────

const traceColumns: ColumnDef<TraceRow>[] = [
  // ─ # (step index) ───────────────────────────────────────────────────────────
  {
    id: "index",
    size: 44,
    header: () => (
      <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
        #
      </span>
    ),
    cell: ({ row }) => (
      <span className="font-mono text-[11px] text-muted-foreground">
        {row.index + 1}
      </span>
    ),
    enableSorting: false,
  },

  // ─ Kind ─────────────────────────────────────────────────────────────────────
  {
    accessorKey: "kind",
    size: 170,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-2 h-8 px-2 text-[11px] font-medium uppercase tracking-wider text-muted-foreground"
        onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
      >
        Kind
        <ArrowUpDown className="ml-1.5 h-3 w-3" />
      </Button>
    ),
    cell: ({ row }) => (
      <div className="flex items-center gap-1.5">
        <span className="shrink-0 text-muted-foreground">
          {kindIcon(row.original.kind)}
        </span>
        <Badge size="sm" variant={kindVariant(row.original.kind)}>
          {prettyKind(row.original.kind)}
        </Badge>
      </div>
    ),
    filterFn: (row, _id, value) =>
      row.original.kind
        .toLowerCase()
        .includes((value as string).toLowerCase()),
  },

  // ─ Summary ──────────────────────────────────────────────────────────────────
  {
    accessorKey: "summary",
    size: 340,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-2 h-8 px-2 text-[11px] font-medium uppercase tracking-wider text-muted-foreground"
        onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
      >
        Summary / Verdict
        <ArrowUpDown className="ml-1.5 h-3 w-3" />
      </Button>
    ),
    cell: ({ row }) => {
      const summary = row.original.summary;
      return (
        <div className="min-w-0" title={summary}>
          <p className="line-clamp-2 text-xs leading-5 text-foreground">
            {summary || "—"}
          </p>
        </div>
      );
    },
    filterFn: (row, _id, value) =>
      row.original.summary
        .toLowerCase()
        .includes((value as string).toLowerCase()),
  },

  // ─ Agent / Path ─────────────────────────────────────────────────────────────
  {
    accessorKey: "agent_id",
    size: 160,
    header: () => (
      <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
        Agent
      </span>
    ),
    cell: ({ row }) => (
      <div className="min-w-0 text-xs">
        <div
          className="truncate font-medium"
          title={row.original.agent_id}
        >
          {row.original.agent_id || "—"}
        </div>
        <div
          className={cn(
            "mt-1 font-mono text-[10px] uppercase tracking-wider",
            row.original.execution_path === "llm"
              ? "text-primary/70"
              : row.original.execution_path === "hybrid"
                ? "text-severity-medium/70"
                : "text-muted-foreground",
          )}
        >
          {formatPath(row.original.execution_path)}
        </div>
      </div>
    ),
    filterFn: (row, _id, value) => {
      const q = (value as string).toLowerCase();
      return (
        row.original.agent_id.toLowerCase().includes(q) ||
        row.original.execution_path.toLowerCase().includes(q)
      );
    },
  },

  // ─ Counts ───────────────────────────────────────────────────────────────────
  {
    accessorKey: "tool_call_count",
    size: 90,
    header: ({ column }) => (
      <Button
        variant="ghost"
        size="sm"
        className="-ml-2 h-8 px-2 text-[11px] font-medium uppercase tracking-wider text-muted-foreground"
        onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
      >
        Tools
        <ArrowUpDown className="ml-1.5 h-3 w-3" />
      </Button>
    ),
    cell: ({ row }) => (
      <div className="text-xs">
        <div className="font-mono text-sm font-semibold">
          {row.original.tool_call_count}
        </div>
        <div className="mt-0.5 font-mono text-[10px] text-muted-foreground">
          {row.original.belief_count} beliefs
        </div>
      </div>
    ),
    filterFn: (row, _id, value) =>
      String(row.original.tool_call_count).includes(value as string),
  },

  // ─ Action ID ────────────────────────────────────────────────────────────────
  {
    accessorKey: "agent_action_id",
    size: 120,
    header: () => (
      <span className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
        Action ID
      </span>
    ),
    cell: ({ row }) => (
      <div
        className="min-w-0 font-mono"
        title={row.original.agent_action_id}
      >
        <div className="truncate text-[11px] text-foreground">
          {row.original.agent_action_id.slice(0, 8)}
        </div>
        <div className="mt-1 truncate text-[10px] text-muted-foreground/60">
          {row.original.agent_action_id.slice(8, 16)}…
        </div>
      </div>
    ),
    filterFn: (row, _id, value) =>
      row.original.agent_action_id
        .toLowerCase()
        .includes((value as string).toLowerCase()),
  },
];

// ── Table ─────────────────────────────────────────────────────────────────────

function TraceTable({ rows }: { rows: TraceRow[] }) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([]);
  const [globalFilter, setGlobalFilter] = useState("");

  const table = useReactTable({
    data: rows,
    columns: traceColumns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    onGlobalFilterChange: setGlobalFilter,
    globalFilterFn: traceGlobalFilter,
    state: { sorting, columnFilters, globalFilter },
    defaultColumn: { minSize: 40, maxSize: 600 },
  });

  const visibleRows = table.getRowModel().rows;

  return (
    <div>
      {/* Toolbar */}
      <div className="flex flex-wrap items-center gap-2 border-b border-border px-3 py-2">
        <Input
          placeholder="Search trace steps…"
          value={globalFilter}
          onChange={(e) => setGlobalFilter(e.target.value)}
          className="h-7 max-w-xs text-xs bg-background/60 border-border/70"
        />
        <Input
          placeholder="Filter by kind…"
          value={(table.getColumn("kind")?.getFilterValue() as string) ?? ""}
          onChange={(e) =>
            table.getColumn("kind")?.setFilterValue(e.target.value)
          }
          className="h-7 max-w-[136px] text-xs bg-background/60 border-border/70"
        />
        <span className="ml-auto font-mono text-[11px] text-muted-foreground">
          {visibleRows.length} step{visibleRows.length !== 1 ? "s" : ""}
        </span>
      </div>

      {/* Table */}
      <div className="max-h-[min(28rem,46vh)] overflow-auto">
        <table className="w-full table-fixed caption-bottom text-sm">
          <thead className="sticky top-0 z-[1] border-b border-border bg-card shadow-sm">
            {table.getHeaderGroups().map((hg) => (
              <tr key={hg.id}>
                {hg.headers.map((header) => (
                  <th
                    key={header.id}
                    className="h-9 px-3 text-left align-middle"
                    style={{ width: header.getSize() }}
                  >
                    {header.isPlaceholder
                      ? null
                      : flexRender(
                          header.column.columnDef.header,
                          header.getContext(),
                        )}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody className="[&_tr:last-child]:border-0">
            {visibleRows.length > 0 ? (
              visibleRows.map((row) => (
                <tr
                  key={row.id}
                  className="border-b border-border/70 transition-colors hover:bg-secondary/40"
                >
                  {row.getVisibleCells().map((cell) => (
                    <td
                      key={cell.id}
                      className="px-3 py-2.5 align-middle"
                      style={{ width: cell.column.getSize() }}
                    >
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext(),
                      )}
                    </td>
                  ))}
                </tr>
              ))
            ) : (
              <tr>
                <td
                  colSpan={traceColumns.length}
                  className="py-6 text-center text-xs text-muted-foreground"
                >
                  No trace steps match the current filter.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="border-t border-border px-3 py-1.5">
        <span className="font-mono text-[11px] text-muted-foreground">
          {visibleRows.length} / {rows.length} step
          {rows.length !== 1 ? "s" : ""}
        </span>
      </div>
    </div>
  );
}

// ── Panel ─────────────────────────────────────────────────────────────────────

export function AttestationTracePanel({
  caseId,
  liveSteps = [],
  wsStatus = "disconnected",
}: {
  caseId: string;
  liveSteps?: KafkaTraceStep[];
  wsStatus?: WsTraceStatus;
}) {
  const [data, setData] = useState<TraceResponse | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(() => {
    setLoading(true);
    void fetchCaseTrace(caseId)
      .then(setData)
      .catch(() => setData({ error: "Failed to load trace" }))
      .finally(() => setLoading(false));
  }, [caseId]);

  useEffect(() => {
    queueMicrotask(() => {
      load();
    });
  }, [load]);

  const rows = useMemo<TraceRow[]>(() => {
    const http = data?.steps ?? [];
    const live = liveSteps.map(kafkaToDisplay);
    const seen = new Set<string>();
    const merged: HttpTraceStep[] = [];
    for (const s of [...live, ...http]) {
      const k = `${s.agent_action_id}-${s.kind}`;
      if (seen.has(k)) continue;
      seen.add(k);
      merged.push(s);
    }
    return merged.map(toTraceRow);
  }, [data?.steps, liveSteps]);

  return (
    <section className="rounded-lg border border-border bg-card/80 shadow-sm">
      <header className="flex items-center justify-between border-b border-border px-3 py-2">
        <div className="flex items-center gap-2">
          <FileWarning className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">Attestation trace</h2>
          <span
            className={cn(
              "font-mono text-[10px]",
              wsStatus === "connected"
                ? "text-signal-good"
                : "text-muted-foreground",
            )}
          >
            WS {wsStatus}
          </span>
        </div>
        <button
          type="button"
          onClick={load}
          className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-[10px] text-muted-foreground hover:bg-secondary"
        >
          <RefreshCw className={cn("h-3 w-3", loading && "animate-spin")} />
          Refresh
        </button>
      </header>

      {loading && !data ? (
        <div className="flex items-center gap-2 px-3 py-4 text-xs text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" />
          Loading trace for case…
        </div>
      ) : data?.error ? (
        <p className="px-3 py-4 text-xs text-amber-600 dark:text-amber-400">
          {data.error}
        </p>
      ) : rows.length === 0 ? (
        <p className="px-3 py-4 text-xs text-muted-foreground">
          No attestation rows found for{" "}
          <code className="rounded bg-secondary px-1">{caseId}</code>.
        </p>
      ) : (
        <TraceTable rows={rows} />
      )}
    </section>
  );
}
