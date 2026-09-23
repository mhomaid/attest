import { StatusBadge } from "@/components/workbench/status-badge";

export default function CoveragePage() {
  return (
    <div className="space-y-3 p-3">
      <div className="rounded-lg border border-border bg-card/80 p-4">
        <div className="mb-2 flex flex-wrap items-center gap-2">
          <StatusBadge tone="info">Coverage</StatusBadge>
        </div>
        <h1 className="text-xl font-semibold tracking-tight">MITRE ATT&CK map</h1>
        <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
          Phase 8 scaffold: primary MITRE coverage view lands here in a later milestone. Navigate via
          the command palette (⌘K → Coverage).
        </p>
      </div>
    </div>
  );
}
