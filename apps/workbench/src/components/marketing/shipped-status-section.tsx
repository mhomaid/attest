const path = [
  {
    n: "01",
    title: "Ingest",
    body: "CloudTrail JSON lands on the collector, becomes OCSF 1.3, and publishes to Kafka.",
  },
  {
    n: "02",
    title: "Detect",
    body: "RisingWave keeps hot views. Ten HELIQL rules compile to streaming SQL and fire alerts.",
  },
  {
    n: "03",
    title: "Triage",
    body: "ONNX XGBoost + SHAP + novelty + a calibration sidecar. About 28 ms, no LLM required.",
  },
  {
    n: "04",
    title: "Attest",
    body: "Every verdict is an Ed25519 envelope. Tools only move through the MCP gateway.",
  },
  {
    n: "05",
    title: "Workbench",
    body: "Live queue, cases, Simulate Lab, Load Lab, and admin health on this site.",
  },
] as const;

const inThisCut = [
  "OCSF collector",
  "Kafka / RisingWave",
  "Warm Parquet on MinIO",
  "ClickHouse s3() query",
  "10 HELIQL detections",
  "ONNX classifier",
  "Calibration sidecar",
  "Ed25519 envelopes",
  "MCP gateway",
  "Queue + cases",
  "Simulate Lab",
  "Better Auth",
] as const;

const notInThisCut = [
  "50-rule catalog",
  "AADF / OTEL GenAI",
  "SIDM detection PRs",
  "OIDC SSO",
  "Hunter agent",
  "Responder autonomy",
  "Shared attestation API",
  "Arroyo CEP",
] as const;

export function ShippedStatusSection() {
  return (
    <section id="status" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          What this demo is
        </p>
        <h2 className="mt-2 max-w-3xl text-3xl font-semibold tracking-tight sm:text-4xl">
          The live path is ingest to a signed verdict. Everything else is labeled.
        </h2>
        <p className="mt-4 max-w-2xl text-muted-foreground">
          This Railway cut is the hot path we actually run: detect, classify,
          sign, and show it here. The twelve-month blueprint stays on the page
          so a visitor is never sold a slide as a ship.
        </p>

        <ol className="mt-12 grid gap-3 sm:grid-cols-2 lg:grid-cols-5">
          {path.map((step, i) => (
            <li
              key={step.n}
              className="relative rounded-2xl border border-border/70 bg-card/40 p-5"
            >
              <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-primary">
                {step.n}
                {i < path.length - 1 ? (
                  <span className="hidden text-muted-foreground lg:inline"> →</span>
                ) : null}
              </p>
              <h3 className="mt-3 text-base font-semibold">{step.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                {step.body}
              </p>
            </li>
          ))}
        </ol>

        <div className="mt-10 grid gap-4 lg:grid-cols-[1.4fr_1fr]">
          <div className="rounded-2xl border border-signal-good/25 bg-signal-good/5 p-6">
            <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-signal-good">
              Running here
            </p>
            <p className="mt-2 text-sm text-muted-foreground">
              Phases 1–7 and the workbench shell. Classifier triage works
              without an LLM; escalation needs a reachable inference endpoint.
            </p>
            <ul className="mt-5 flex flex-wrap gap-2">
              {inThisCut.map((item) => (
                <li
                  key={item}
                  className="rounded-full border border-signal-good/20 bg-background/40 px-3 py-1 text-xs text-foreground"
                >
                  {item}
                </li>
              ))}
            </ul>
          </div>

          <div className="rounded-2xl border border-border/70 bg-card/30 p-6">
            <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
              Labeled, not claimed
            </p>
            <p className="mt-2 text-sm text-muted-foreground">
              Specified in the docs. Not in this Railway cut.
            </p>
            <ul className="mt-5 flex flex-wrap gap-2">
              {notInThisCut.map((item) => (
                <li
                  key={item}
                  className="rounded-full border border-border/60 px-3 py-1 text-xs text-muted-foreground"
                >
                  {item}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </section>
  );
}
