import { Check } from "lucide-react";

const cases = [
  {
    title: "Stream-first ingest with intentional shaping",
    body: "Normalize to OCSF at the edge, enrich in flight, route high-signal data to live detections in seconds while cold data lands in customer Iceberg.",
  },
  {
    title: "Portable detection across time horizons",
    body: "One HELIQL rule: real-time streams, 30-day retro hunts, and federated push-down — no triplicate authoring.",
  },
  {
    title: "Autonomous triage with a verifiable verdict",
    body: "Classifier and LLM paths both emit signed envelopes: data sources, tools, model versions, calibrated confidence, and replay for compliance.",
  },
  {
    title: "Agent-aware detection of rogue internal AI",
    body: "Baseline tool invocation, correlate GenAI traces with identity and egress, block after deterministic shadow checks.",
  },
  {
    title: "Closed-loop detection improvement",
    body: "Precision, recall, and drift inform the Detection Engineer agent’s next PR — humans stay in control via review.",
  },
] as const;

export function UseCasesSection() {
  return (
    <section id="use-cases" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Key use cases
        </p>
        <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
          How teams use Attest
        </h2>
        <p className="mt-4 max-w-3xl text-muted-foreground">
          Summarized from the product requirements: detection engineering, CISO auditability, SOC
          throughput, AI platform risk, and MSSP scale.
        </p>

        <ul className="mt-10 space-y-3">
          {cases.map((uc) => (
            <li
              key={uc.title}
              className="flex gap-3 rounded-xl border border-border/70 bg-card/40 px-4 py-3"
            >
              <span className="mt-0.5 grid h-6 w-6 shrink-0 place-items-center rounded-md border border-signal-good/30 bg-signal-good/10 text-signal-good">
                <Check className="h-3.5 w-3.5" />
              </span>
              <div>
                <p className="font-medium">{uc.title}</p>
                <p className="mt-1 text-sm text-muted-foreground">{uc.body}</p>
              </div>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
