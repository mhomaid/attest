import { FlowRail } from "@/components/marketing/flow-diagram";

const path = [
  {
    label: "Ingest",
    hint: "OCSF → Kafka",
    detail: "CloudTrail JSON lands on the collector, becomes OCSF 1.3, and publishes to Kafka.",
  },
  {
    label: "Detect",
    hint: "10 HELIQL rules",
    detail: "RisingWave keeps hot views. Ten HELIQL rules compile to streaming SQL and fire alerts.",
  },
  {
    label: "Triage",
    hint: "<5 ms ONNX",
    detail: "ONNX XGBoost + SHAP + novelty + a calibration sidecar. Inference under 5 ms, no LLM required.",
  },
  {
    label: "Attest",
    hint: "Ed25519",
    detail: "Every verdict is an Ed25519 envelope. Tools only move through the MCP gateway.",
  },
  {
    label: "Workbench",
    hint: "This site",
    detail: "Live queue, cases, Simulate Lab, Load Lab, and admin health on this site.",
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

        <FlowRail steps={path} className="mt-12" cycleMs={1900} />

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
