"use client";

import { Crosshair, FlaskConical, Loader2, Search } from "lucide-react";
import { useState } from "react";
import { StatusBadge } from "@/components/workbench/status-badge";
import { analytics } from "@/lib/analytics";

const HELIQL_PLACEHOLDER = `detection: my_hunt_hypothesis
where:
  - event.api_operation = "CreateAccessKey"
  - event.actor_user_name NOT IN baseline(identity.user, 30d)
severity: high`;

export default function HuntPage() {
  const [queryText, setQueryText] = useState("");
  const [sql, setSql] = useState("SELECT 1 AS ok");
  const [result, setResult] = useState<unknown>(null);
  const [error, setError]   = useState<string | null>(null);
  const [busy, setBusy]     = useState(false);

  async function runWarmTier() {
    setBusy(true);
    setError(null);
    setResult(null);
    const trimmed = sql.trim();
    if (!trimmed.toLowerCase().startsWith("select")) {
      setError("Warm tier API accepts read-only ClickHouse SQL starting with SELECT.");
      setBusy(false);
      return;
    }
    try {
      const res = await fetch("/api/warm/query", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ sql: trimmed }),
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        setError(typeof data === "object" && data && "error" in data
          ? String((data as { error: unknown }).error)
          : `HTTP ${res.status}`);
        return;
      }
      analytics.hunt_query_run();
      setResult(data);
    } catch {
      setError("Request failed — is the workbench dev server and control-plane up?");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-3 p-3">
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="mb-2 flex items-center gap-2">
          <StatusBadge tone="info">Hunt</StatusBadge>
          <StatusBadge tone="muted">Ad-hoc query</StatusBadge>
        </div>
        <h1 className="text-xl font-semibold tracking-tight">Threat Hunting</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          HELIQL editor below is for drafting hypotheses. Warm-tier execution uses read-only ClickHouse{" "}
          <code className="rounded bg-secondary px-1 text-[11px]">SELECT</code> via{" "}
          <code className="rounded bg-secondary px-1 text-[11px]">POST /v1/warm/query</code>.
        </p>
      </section>

      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="mb-2 flex items-center gap-2">
          <Search className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">HELIQL hypothesis (draft)</h2>
        </div>
        <textarea
          rows={4}
          value={queryText}
          onChange={(e) => setQueryText(e.target.value)}
          placeholder={HELIQL_PLACEHOLDER}
          className="w-full resize-none rounded-md border border-border bg-background/60 px-3 py-2 font-mono text-xs text-foreground placeholder:text-muted-foreground/40 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30"
        />
      </section>

      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="mb-2 flex items-center gap-2">
          <DatabaseGlyph />
          <h2 className="text-sm font-semibold">ClickHouse SQL — warm tier</h2>
        </div>
        <textarea
          rows={3}
          value={sql}
          onChange={(e) => setSql(e.target.value)}
          placeholder='SELECT 1'
          className="w-full resize-none rounded-md border border-border bg-background/60 px-3 py-2 font-mono text-xs text-foreground placeholder:text-muted-foreground/40 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30"
        />
        <div className="mt-2 flex flex-wrap gap-2">
          <button
            type="button"
            disabled={busy}
            onClick={() => void runWarmTier()}
            className="inline-flex items-center gap-1.5 rounded-md bg-primary px-4 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-60"
          >
            {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null}
            Run against warm tier
          </button>
          <button
            type="button"
            disabled
            title="Stream path not wired yet"
            className="rounded-md border border-border px-4 py-1.5 text-xs text-muted-foreground opacity-50"
          >
            Run against live stream
          </button>
          <button
            type="button"
            disabled
            title="Hypothesis persistence not wired yet"
            className="rounded-md border border-border px-4 py-1.5 text-xs text-muted-foreground opacity-50"
          >
            Save hypothesis
          </button>
        </div>
        {error ? (
          <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">{error}</p>
        ) : null}
        {result !== null ? (
          <pre className="mt-3 max-h-64 overflow-auto rounded-md border border-border/80 bg-background/80 p-2 text-[11px] leading-relaxed text-muted-foreground">
            {JSON.stringify(result, null, 2)}
          </pre>
        ) : null}
      </section>

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
          No saved hypotheses yet. Draft HELIQL above; execute warm-tier SQL in the middle panel.
        </div>
      </section>
    </div>
  );
}

function DatabaseGlyph() {
  return (
    <svg className="h-4 w-4 text-muted-foreground" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <ellipse cx="12" cy="5" rx="9" ry="3" />
      <path d="M3 5v6c0 1.7 4 3 9 3s9-1.3 9-3V5" />
      <path d="M3 11v6c0 1.7 4 3 9 3s9-1.3 9-3v-6" />
    </svg>
  );
}
