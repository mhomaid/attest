import { VerticalFlow } from "@/components/marketing/flow-diagram";

const planes = [
  {
    id: "workbench",
    name: "Workbench plane",
    stack: "Next.js 16 · Better Auth · Zustand · WebSocket",
    body: "SOC console: live queue, case investigation, SHAP + attestation replay, Simulate Lab, Load Lab, admin health.",
  },
  {
    id: "agentic",
    name: "Agentic plane (VAS)",
    stack: "orchestrator · MCP gateway · policy engine · Ed25519",
    body: "Hybrid Triager (XGBoost + LLM) and Investigator. Agents call tools only through the gateway. Every verdict is a signed envelope.",
  },
  {
    id: "detection",
    name: "Detection plane",
    stack: "HELIQL · RisingWave MVs · Arroyo CEP · Sigma import",
    body: "Write a rule once. Compile to streaming SQL for the hot path, or SELECT over Iceberg for hunts. Ten MITRE-mapped rules ship in-repo.",
  },
  {
    id: "storage",
    name: "Storage plane",
    stack: "RisingWave hot · Parquet / Iceberg warm · ClickHouse",
    body: "Hot views for last-N events and 30-day baselines. Warm Parquet on MinIO (S3 in prod). ClickHouse reads the same files; there is no ETL copy.",
  },
  {
    id: "streaming",
    name: "Streaming plane",
    stack: "Rust collector · OCSF 1.3 · Kafka / Redpanda",
    body: "CloudTrail JSON in, FlatEvent out. Topic per source class, partitioned by tenant. Same Kafka protocol locally and on Railway.",
  },
  {
    id: "control",
    name: "Control plane",
    stack: "axum 0.8 · Postgres · Railway / Terraform BYOC",
    body: "REST + WS for events, baselines, warm SQL, fired detections, metrics. Identity today is Better Auth on Postgres; OIDC is GA.",
  },
] as const;

const hops = [
  { from: "CloudTrail JSON", via: "POST /ingest", to: "attest-collector :4000" },
  { from: "OCSF FlatEvent", via: "rdkafka", to: "topic cloudtrail" },
  { from: "cloudtrail", via: "Kafka source", to: "RisingWave recent_events + entity_baselines" },
  { from: "cloudtrail", via: "batch 1k / 30s", to: "MinIO attest-warm/*.parquet" },
  { from: "*.heliql", via: "CREATE MATERIALIZED VIEW", to: "topic alerts" },
  { from: "alert", via: "POST /triage", to: "ONNX → calibrate → attest → workbench" },
] as const;

export function ArchitectureSection() {
  return (
    <section id="architecture" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Architecture
        </p>
        <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
          Six planes, one contract
        </h2>
        <p className="mt-4 max-w-3xl text-muted-foreground">
          The agentic plane never opens a database or a cloud credential. It
          calls a governed query API. That is what makes every decision
          auditable: every byte an agent touches is mediated, hashed, and
          signed.
        </p>

        <ol className="mt-10 space-y-2">
          {planes.map((plane, i) => (
            <li
              key={plane.id}
              className="grid gap-3 rounded-xl border border-border/70 bg-card/40 px-4 py-4 sm:grid-cols-[auto_1fr] sm:items-start"
            >
              <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-primary">
                {String(i + 1).padStart(2, "0")}
              </span>
              <div>
                <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                  <h3 className="text-sm font-semibold">{plane.name}</h3>
                  <p className="font-mono text-[11px] text-muted-foreground">
                    {plane.stack}
                  </p>
                </div>
                <p className="mt-1.5 text-sm leading-relaxed text-muted-foreground">
                  {plane.body}
                </p>
              </div>
            </li>
          ))}
        </ol>

        <h3 className="mt-14 text-lg font-semibold tracking-tight">
          Path of a single ConsoleLogin
        </h3>
        <p className="mt-2 max-w-3xl text-sm text-muted-foreground">
          One event fans out to the hot tier, the warm lake, the detection
          runtime, and, if a rule fires, the hybrid triager. The workbench
          reads the same artifacts; it does not keep a second copy of truth.
        </p>
        <VerticalFlow hops={hops} className="mt-6 max-w-2xl" />
      </div>
    </section>
  );
}
