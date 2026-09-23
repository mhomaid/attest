import type { Metadata } from "next";
import { ArrowUpRight } from "lucide-react";
import { VerticalFlow } from "@/components/marketing/flow-diagram";
import { MarketingFooter } from "@/components/marketing/marketing-footer";
import { MarketingNav } from "@/components/marketing/marketing-nav";
import { PipelineDiagram } from "@/components/marketing/pipeline-diagram";
import { REPO_URL, repoLink, stackLayers } from "@/components/marketing/stack-data";

export const metadata: Metadata = {
  title: "Attest stack: every component and where it lives",
  description:
    "The full Attest stack: Rust collector, Kafka, RisingWave, Arroyo, HELIQL, Iceberg, ClickHouse, Postgres 18, ONNX classifier, MCP gateway, policy engine, Ed25519 attestation and the Next.js workbench.",
};

const hops = [
  { from: "CloudTrail JSON", via: "POST /ingest", to: "attest-collector :4000" },
  { from: "OCSF event", via: "rdkafka", to: "topic cloudtrail" },
  { from: "cloudtrail", via: "Kafka source", to: "RisingWave recent_events + entity_baselines" },
  { from: "cloudtrail", via: "batch → Parquet", to: "Iceberg warm tier on MinIO / S3" },
  { from: "*.heliql", via: "CREATE MATERIALIZED VIEW", to: "topic alerts" },
  { from: "alert", via: "POST /triage", to: "ONNX → policy → Ed25519 envelope → workbench" },
] as const;

export default function StackPage() {
  const total = stackLayers.reduce((n, l) => n + l.components.length, 0);

  return (
    <>
      <MarketingNav />
      <main className="pt-28">
        <section className="border-b border-border/60 pb-16">
          <div className="mx-auto max-w-6xl px-4 sm:px-6">
            <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
              The stack
            </p>
            <h1 className="mt-3 max-w-3xl text-3xl font-bold tracking-tight sm:text-4xl">
              {total} components across {stackLayers.length} layers, all in{" "}
              <a href={REPO_URL} target="_blank" rel="noreferrer" className="text-primary hover:underline">
                one repo
              </a>
              .
            </h1>
            <nav aria-label="Layers" className="mt-8 flex flex-wrap gap-1.5">
              {stackLayers.map((l, i) => (
                <a
                  key={l.id}
                  href={`#${l.id}`}
                  className="rounded-full border border-border/70 px-3 py-1 font-mono text-[10px] uppercase tracking-wider text-muted-foreground transition-colors hover:border-primary/50 hover:text-foreground"
                >
                  {String(i + 1).padStart(2, "0")} {l.name}
                </a>
              ))}
            </nav>
            <div className="mt-10 rounded-2xl border border-border/70 bg-card/30 p-4 sm:p-6">
              <PipelineDiagram />
            </div>
          </div>
        </section>

        {stackLayers.map((layer, i) => (
          <section key={layer.id} id={layer.id} className="scroll-mt-16 border-b border-border/60 py-14">
            <div className="mx-auto grid max-w-6xl gap-6 px-4 sm:px-6 lg:grid-cols-[14rem_minmax(0,1fr)]">
              <div>
                <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-muted-foreground">
                  Layer {String(i + 1).padStart(2, "0")}
                </p>
                <h2 className="mt-1 text-xl font-semibold tracking-tight">{layer.name}</h2>
                <p className="mt-2 text-sm text-muted-foreground">{layer.summary}</p>
              </div>
              <ul className="grid gap-3 sm:grid-cols-2">
                {layer.components.map((c) => (
                  <li key={c.name}>
                    <a
                      href={repoLink(c.path)}
                      target="_blank"
                      rel="noreferrer"
                      className="group flex h-full flex-col rounded-xl border border-border/70 bg-card/40 p-4 transition-colors hover:border-primary/50"
                    >
                      <span className="flex items-start justify-between gap-2">
                        <span className="font-semibold">{c.name}</span>
                        <ArrowUpRight className="h-4 w-4 shrink-0 text-muted-foreground transition-colors group-hover:text-primary" />
                      </span>
                      <span className="mt-0.5 font-mono text-[11px] text-primary/90">{c.tech}</span>
                      <span className="mt-2 text-sm leading-relaxed text-muted-foreground">{c.role}</span>
                      <span className="mt-3 font-mono text-[10px] text-muted-foreground/70">{c.path}</span>
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          </section>
        ))}

        <section className="py-16">
          <div className="mx-auto max-w-6xl px-4 sm:px-6">
            <h2 className="text-xl font-semibold tracking-tight">Path of a single ConsoleLogin</h2>
            <p className="mt-2 max-w-3xl text-sm text-muted-foreground">
              One event fans out to the hot tier, the warm lake and the detection runtime. If a rule
              fires, it ends as a signed envelope in the workbench.
            </p>
            <VerticalFlow hops={hops} className="mt-6 max-w-2xl" />
          </div>
        </section>
      </main>
      <MarketingFooter />
    </>
  );
}
