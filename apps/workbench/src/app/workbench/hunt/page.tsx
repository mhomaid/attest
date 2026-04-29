import { Crosshair, FlaskConical, Search } from "lucide-react";
import { StatusBadge } from "@/components/workbench/status-badge";

export default function HuntPage() {
  const savedHunts = [
    {
      id: "hunt-001",
      name: "Lateral movement via IAM key creation",
      query: "SELECT * WHERE api_operation = 'CreateAccessKey' AND actor_user_name NOT IN baseline(identity.user, 30d)",
      lastRun: "2h ago",
      hits: 3,
    },
    {
      id: "hunt-002",
      name: "Cross-region activity burst",
      query: "SELECT actor_user_name, count(distinct cloud_region) WHERE auth_status = 'Success' GROUP BY actor_user_name HAVING count > 3",
      lastRun: "6h ago",
      hits: 0,
    },
    {
      id: "hunt-003",
      name: "S3 mass exfil hypothesis",
      query: "SELECT actor_user_name, count(*) WHERE api_operation = 'GetObject' GROUP BY actor_user_name ORDER BY count DESC",
      lastRun: "1d ago",
      hits: 7,
    },
  ];

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

      {/* Saved hunts */}
      <section className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm">
        <header className="flex items-center gap-2 border-b border-border bg-card/95 px-3 py-2">
          <FlaskConical className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">Saved Hypotheses</h2>
          <span className="rounded bg-secondary px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
            {savedHunts.length}
          </span>
        </header>
        <div className="divide-y divide-border/80">
          {savedHunts.map((hunt) => (
            <div key={hunt.id} className="px-3 py-3 transition-colors hover:bg-secondary/40">
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <Crosshair className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                    <span className="truncate text-sm font-medium">{hunt.name}</span>
                    {hunt.hits > 0 && (
                      <StatusBadge tone="high">{hunt.hits} hit{hunt.hits !== 1 ? "s" : ""}</StatusBadge>
                    )}
                  </div>
                  <pre className="mt-1 truncate font-mono text-[10px] text-muted-foreground/60">
                    {hunt.query}
                  </pre>
                </div>
                <div className="shrink-0 text-right">
                  <div className="font-mono text-[11px] text-muted-foreground">{hunt.lastRun}</div>
                  <div className="mt-1 flex gap-1 justify-end">
                    <button className="rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground hover:bg-secondary/50">
                      Re-run
                    </button>
                    <button className="rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground hover:bg-secondary/50">
                      → Detection
                    </button>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
