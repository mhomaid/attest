"use client";

import { motion } from "framer-motion";
import {
  BrainCircuit,
  Cpu,
  Database,
  FileCode2,
  Globe,
  Key,
  Layers,
  Lock,
  Network,
  Server,
  Warehouse,
  Workflow,
} from "lucide-react";

const stack = [
  // ── Ingest & Normalisation ──────────────────────────────────────
  {
    category: "Ingest",
    icon: Cpu,
    label: "attest-collector",
    detail: "Rust 1.95 · CloudTrail → OCSF 1.3 normalizer · static binary <50 MB",
  },
  {
    category: "Ingest",
    icon: Lock,
    label: "OCSF 1.3",
    detail: "Class 3002 Authentication · Class 6003 Cloud API Activity · canonical event enum",
  },
  // ── Stream Processing ───────────────────────────────────────────
  {
    category: "Streaming",
    icon: Workflow,
    label: "Redpanda",
    detail: "Kafka-compatible broker · topic: cloudtrail · partitioned by tenant_id",
  },
  {
    category: "Streaming",
    icon: Database,
    label: "RisingWave",
    detail: "Streaming SQL · entity_baselines + recent_events materialized views",
  },
  // ── Warm Tier ───────────────────────────────────────────────────
  {
    category: "Storage",
    icon: Layers,
    label: "Apache Iceberg",
    detail: "Parquet on MinIO (local) / S3 (prod) · daily partitioning by tenant_id",
  },
  {
    category: "Storage",
    icon: Warehouse,
    label: "ClickHouse",
    detail: "s3() over warm Parquet · sub-30 s aggregate queries · warm tier API",
  },
  // ── Detection ───────────────────────────────────────────────────
  {
    category: "Detection",
    icon: FileCode2,
    label: "HELIQL DSL",
    detail: "Streaming detection language · compiles to RisingWave SQL · Sigma-compatible",
  },
  {
    category: "Detection",
    icon: Network,
    label: "Detection Mesh",
    detail: "10 reference detections · backtest CI · detection-as-code PR workflow",
  },
  // ── AI Triage ───────────────────────────────────────────────────
  {
    category: "AI / Triage",
    icon: BrainCircuit,
    label: "Hybrid Triager",
    detail: "XGBoost + ONNX <5 ms primary · Claude Sonnet / Qwen 3 7B escalation",
  },
  {
    category: "AI / Triage",
    icon: Server,
    label: "MCP Gateway",
    detail: "Rust · tool attestation & interception · policy-engine RBAC per agent role",
  },
  // ── Platform ────────────────────────────────────────────────────
  {
    category: "Platform",
    icon: Key,
    label: "Ed25519 Attestation",
    detail: "Signed reasoning traces · Classifier / LLM / Hybrid envelope variants",
  },
  {
    category: "Platform",
    icon: Globe,
    label: "Next.js 16 + Bun",
    detail: "App Router workbench · Better Auth · Zustand · shadcn / Tailwind · Railway",
  },
  {
    category: "Platform",
    icon: Database,
    label: "Postgres 18",
    detail: "Better Auth sessions · Alembic migrations in infra/db · Arroyo pipeline state",
  },
  {
    category: "AI / Triage",
    icon: BrainCircuit,
    label: "tract-onnx + uv",
    detail: "In-process XGBoost · Mahalanobis novelty · Python calibration sidecar :5001",
  },
  {
    category: "Streaming",
    icon: Workflow,
    label: "Arroyo",
    detail: "CEP sequences (login → S3) + Parquet ETL · UI :5115 · SQL pipelines in-repo",
  },
];

export function SubstrateSection() {
  return (
    <section id="substrate" className="border-b border-border/60 py-24">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        {/* Header */}
        <div className="mb-14 text-center">
          <h2 className="text-3xl font-bold tracking-tight sm:text-4xl lg:text-5xl">
            Our Technology
          </h2>
          <p className="mx-auto mt-4 max-w-2xl text-muted-foreground">
            A streaming-first, attestation-native stack built end-to-end in Rust
            and deployed on Railway, from raw CloudTrail to verifiable AI verdicts.
          </p>
        </div>

        <motion.ul
          initial="hidden"
          whileInView="show"
          viewport={{ once: true, margin: "-60px" }}
          variants={{
            hidden: {},
            show: { transition: { staggerChildren: 0.045 } },
          }}
          className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3"
        >
          {stack.map((row) => (
            <motion.li
              key={row.label}
              variants={{
                hidden: { opacity: 0, y: 14 },
                show: {
                  opacity: 1,
                  y: 0,
                  transition: { duration: 0.4, ease: [0.22, 1, 0.36, 1] },
                },
              }}
              className="group flex gap-3 rounded-xl border border-border/60 bg-card/40 p-5 backdrop-blur-sm transition-colors hover:border-primary/40 hover:bg-card/70"
            >
              <div className="mt-0.5 grid h-8 w-8 shrink-0 place-items-center rounded-lg border border-border/70 bg-secondary/60">
                <row.icon className="h-4 w-4 text-primary" />
              </div>
              <div>
                <div className="flex items-center gap-2">
                  <p className="font-semibold leading-snug">{row.label}</p>
                  <span className="rounded-sm border border-border/50 px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-wide text-muted-foreground/70">
                    {row.category}
                  </span>
                </div>
                <p className="mt-1 text-sm leading-relaxed text-muted-foreground">
                  {row.detail}
                </p>
              </div>
            </motion.li>
          ))}
        </motion.ul>
      </div>
    </section>
  );
}
