import { Crosshair, FlaskConical, Search } from "lucide-react";
import { StatusBadge } from "@/components/workbench/status-badge";

export default function HuntPage() {
  return (
    <div className="space-y-3 p-3">
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="mb-2 flex items-center gap-2">
          <StatusBadge tone="info">Hunt</StatusBadge>
          <StatusBadge tone="muted">Ad-hoc query</StatusBadge>
        </div>
        <h1 className="text-xl font-semibold tracking-tight">Threat Hunting</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Run ad-hoc HELIQL queries against the warm tier or live stream. Results are saved as
          hypotheses and can be promoted to detection rules.
        </p>
      </section>

      {/* Query box */}
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="mb-2 flex items-center gap-2">
          <Search className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">New Hunt Query</h2>
        </div>
        <textarea
          rows={4}
          placeholder={`detection: my_hunt_hypothesis
where:
  - event.api_operation = "CreateAccessKey"
  - event.actor_user_name NOT IN baseline(identity.user, 30d)
severity: high`}
          className="w-full resize-none rounded-md border border-border bg-background/60 px-3 py-2 font-mono text-xs text-foreground placeholder:text-muted-foreground/40 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30"
        />
        <div className="mt-2 flex gap-2">
          <button className="rounded-md bg-primary px-4 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-opacity">
            Run against warm tier
          </button>
          <button className="rounded-md border border-border px-4 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-secondary/50">
            Run against live stream
          </button>
          <button className="rounded-md border border-border px-4 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-secondary/50">
            Save hypothesis
          </button>
        </div>
      </section>

      {/* Saved hunts — empty until backend wired (Phase 7) */}
      <section className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm">
        <header className="flex items-center gap-2 border-b border-border bg-card/95 px-3 py-2">
          <FlaskConical className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">Saved Hypotheses</h2>
          <span className="rounded bg-secondary px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
            0
          </span>
        </header>
        <div className="px-3 py-8 text-center text-sm text-muted-foreground">
          <Crosshair className="mx-auto mb-2 h-6 w-6 opacity-30" />
          No saved hypotheses yet. Write a query above and click{" "}
          <span className="font-medium">Save hypothesis</span> to store it here.
        </div>
      </section>
    </div>
  );
}
