/** Industry context cited in product docs, not a performance claim about Attest. */
export function LandscapeSection() {
  return (
    <section className="border-b border-border/60 bg-secondary/20 py-10">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="text-center text-sm font-semibold tracking-wide text-foreground">
          Why the problem space is urgent
        </p>
        <div className="mt-6 grid gap-6 sm:grid-cols-3">
          {[
            { value: "< 2 days", label: "Mean time to exploit (2026, industry trend)" },
            { value: "~25 min", label: "Agentic ransomware chains (reported ranges)" },
            { value: "90%+", label: "Incidents involving identity, including AI agents" },
          ].map((stat) => (
            <div
              key={stat.label}
              className="rounded-xl border border-border/50 bg-background/40 px-4 py-5 text-center"
            >
              <p className="metric-tabular text-2xl font-semibold text-foreground sm:text-3xl">
                {stat.value}
              </p>
              <p className="mt-2 text-xs leading-relaxed text-muted-foreground">{stat.label}</p>
            </div>
          ))}
        </div>
        <p className="mt-6 text-center text-xs text-muted-foreground">
          Figures reflect the threat landscape described in the Attest blueprint; they are not
          product SLAs.
        </p>
      </div>
    </section>
  );
}
