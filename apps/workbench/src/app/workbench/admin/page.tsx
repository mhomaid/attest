import { Activity, Database, Layers, Server, Users } from "lucide-react";
import { StatusBadge } from "@/components/workbench/status-badge";

const CP_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";
const ARROYO_URL = process.env.ARROYO_URL ?? "http://localhost:5115";

async function fetchHealth(): Promise<{
  controlPlane: "ok" | "error";
  arroyo:       "ok" | "error";
  arroyoPipelines: { name: string; state: string }[];
}> {
  const results = await Promise.allSettled([
    fetch(`${CP_URL}/healthz`, { cache: "no-store", signal: AbortSignal.timeout(3000) }),
    fetch(`${ARROYO_URL}/api/v1/ping`, { cache: "no-store", signal: AbortSignal.timeout(3000) }),
    fetch(`${ARROYO_URL}/api/v1/pipelines`, { cache: "no-store", signal: AbortSignal.timeout(3000) }),
  ]);

  const controlPlane = results[0].status === "fulfilled" && results[0].value.ok ? "ok" : "error";
  const arroyo       = results[1].status === "fulfilled" && results[1].value.ok ? "ok" : "error";

  let arroyoPipelines: { name: string; state: string }[] = [];
  if (results[2].status === "fulfilled" && results[2].value.ok) {
    try {
      const body = await results[2].value.json();
      arroyoPipelines = (body.data ?? []).map((p: { name: string }) => ({
        name:  p.name,
        state: "Running",
      }));
    } catch { /* ignore */ }
  }

  return { controlPlane, arroyo, arroyoPipelines };
}

const INFO_ROWS = [
  { label: "Platform version",        value: "0.1.0-alpha" },
  { label: "Streaming engine",        value: "RisingWave 2.x + Arroyo (Rust)" },
  { label: "Warm tier",               value: "MinIO · Parquet via Arroyo ETL" },
  { label: "Query engine",            value: "ClickHouse" },
  { label: "Kafka",                   value: "Confluent KRaft (Railway) / Redpanda (local)" },
  { label: "Attestation scheme",      value: "Ed25519 · SHAP feature attribution" },
  { label: "Classifier",             value: "XGBoost + isotonic calibration" },
  { label: "LLM escalation model",   value: "Claude Sonnet / Qwen 3 32B (air-gapped)" },
];

export default async function AdminPage() {
  const { controlPlane, arroyo, arroyoPipelines } = await fetchHealth();

  const services = [
    { name: "control-plane",    status: controlPlane, port: "8080", icon: Server },
    { name: "arroyo",           status: arroyo,       port: "5115", icon: Activity },
    { name: "collector",        status: "unknown" as const, port: "4000", icon: Layers },
    { name: "storage-iceberg",  status: "unknown" as const, port: "—",    icon: Database },
    { name: "detection-runtime",status: "unknown" as const, port: "—",    icon: Server },
  ];

  const tonemap = { ok: "good", error: "high", unknown: "muted" } as const;

  return (
    <div className="space-y-3 p-3">
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="mb-2 flex items-center gap-2">
          <StatusBadge tone="info">Admin</StatusBadge>
        </div>
        <h1 className="text-xl font-semibold tracking-tight">Platform Administration</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Service health, Arroyo pipeline state, and platform metadata.
        </p>
      </section>

      {/* Service health */}
      <section className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm">
        <header className="flex items-center gap-2 border-b border-border bg-card/95 px-3 py-2">
          <Server className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">Service Health</h2>
          <span className="font-mono text-[10px] text-muted-foreground">live check</span>
        </header>
        <div className="divide-y divide-border/80">
          {services.map(({ name, status, port, icon: Icon }) => (
            <div key={name} className="flex items-center gap-3 px-3 py-2.5">
              <Icon className="h-4 w-4 shrink-0 text-muted-foreground" />
              <code className="flex-1 font-mono text-xs">{name}</code>
              <span className="font-mono text-[11px] text-muted-foreground">:{port}</span>
              <StatusBadge tone={tonemap[status as keyof typeof tonemap]}>
                {status}
              </StatusBadge>
            </div>
          ))}
        </div>
      </section>

      {/* Arroyo pipeline status */}
      <section className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm">
        <header className="flex items-center gap-2 border-b border-border bg-card/95 px-3 py-2">
          <Activity className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">Arroyo Pipelines</h2>
        </header>
        <div className="divide-y divide-border/80">
          {arroyoPipelines.length === 0 ? (
            <p className="px-3 py-4 text-sm text-muted-foreground">
              {arroyo === "error"
                ? "Arroyo unreachable — pipelines not loaded."
                : "No pipelines deployed yet. Run `make arroyo-deploy` or wait for arroyo-deployer."}
            </p>
          ) : (
            arroyoPipelines.map((p) => (
              <div key={p.name} className="flex items-center gap-3 px-3 py-2.5">
                <Activity className="h-4 w-4 shrink-0 text-muted-foreground" />
                <code className="flex-1 font-mono text-xs">{p.name}</code>
                <StatusBadge tone={p.state === "Running" ? "good" : "medium"}>{p.state}</StatusBadge>
              </div>
            ))
          )}
        </div>
      </section>

      {/* Platform info */}
      <section className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm">
        <header className="flex items-center gap-2 border-b border-border bg-card/95 px-3 py-2">
          <Users className="h-4 w-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">Platform Info</h2>
        </header>
        <div className="divide-y divide-border/80">
          {INFO_ROWS.map(({ label, value }) => (
            <div key={label} className="flex items-start gap-3 px-3 py-2">
              <span className="w-44 shrink-0 text-xs text-muted-foreground">{label}</span>
              <span className="text-xs font-medium">{value}</span>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
