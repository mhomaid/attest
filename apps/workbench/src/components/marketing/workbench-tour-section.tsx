import Link from "next/link";

const pages = [
  {
    href: "/workbench/queue",
    name: "Alert queue",
    live: "control-plane /v1/detections/fired + WS /v1/ws/alerts",
    body: "Default landing. Live badge, Arroyo pipeline chips, virtualized rows. Does not auto-scroll when new alerts arrive.",
  },
  {
    href: "/workbench/cases",
    name: "Cases",
    live: "Same fired detections, Open vs Auto-Closed",
    body: "Severity, actor, region, confidence, execution path. Row click uses the event UUID as the case id.",
  },
  {
    href: "/workbench/queue",
    name: "Case workbench",
    live: "hot event + 30-day baseline + POST /triage + attestation trace",
    body: "Timeline of collector → RisingWave → orchestrator → Ed25519. SHAP bars for classifier path. Investigator stepper when escalated.",
  },
  {
    href: "/workbench/detections",
    name: "Detections",
    live: "Last-fired timestamps from the control-plane",
    body: "The ten bundled HELIQL rules with MITRE technique, severity, and a live “last fired” column.",
  },
  {
    href: "/workbench/simulate",
    name: "Simulate Lab",
    live: "collector :4000 + orchestrator :4300",
    body: "Five named attack scenarios, editable actor/region, single-shot and 50× concurrent batch. Memory panel polls all four sinks.",
  },
  {
    href: "/workbench/load",
    name: "Load Lab",
    live: "WS /v1/metrics/stream at 1 Hz",
    body: "Smoke / Sustained / Burst / 1M Challenge. Charts for events/sec, consumer lag, detections/sec, storage rows/sec.",
  },
  {
    href: "/workbench/agents",
    name: "Agents",
    live: "orchestrator /metrics",
    body: "Triager latency percentiles and envelope coverage. Investigator marked live when MCP is up. Hunter and Detection Engineer are labeled as shells.",
  },
  {
    href: "/workbench/hunt",
    name: "Hunt",
    live: "POST /v1/warm/query",
    body: "HELIQL-style editor against ClickHouse over Iceberg. Stream/save and the Hunter agent are placeholders.",
  },
  {
    href: "/workbench/admin",
    name: "Admin",
    live: "healthz on control-plane, collector, Arroyo",
    body: "Service table with Scalar /docs links. Use this page — not Settings — for live health.",
  },
] as const;

export function WorkbenchTourSection() {
  return (
    <section id="workbench" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Analyst console
        </p>
        <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
          Every workbench surface
        </h2>
        <p className="mt-4 max-w-3xl text-muted-foreground">
          Next.js App Router. Server Components fetch live data; if the
          backend is down the page says so. Client islands (queue WS, Simulate,
          Load) keep cross-page state in Zustand.
        </p>

        <ul className="mt-10 divide-y divide-border/60 rounded-xl border border-border/70 bg-card/30">
          {pages.map((page) => (
            <li key={page.name} className="grid gap-2 px-4 py-4 sm:grid-cols-[11rem_1fr]">
              <div>
                <Link
                  href={page.href}
                  className="text-sm font-semibold text-foreground underline-offset-4 hover:underline"
                >
                  {page.name}
                </Link>
                <p className="mt-1 font-mono text-[10px] leading-relaxed text-muted-foreground">
                  {page.live}
                </p>
              </div>
              <p className="text-sm leading-relaxed text-muted-foreground">
                {page.body}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
