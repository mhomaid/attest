"use client";

import { motion } from "framer-motion";
import { Cpu, Database, Globe, Lock, Server, Workflow } from "lucide-react";

const stack = [
  {
    icon: Cpu,
    label: "attest-collector — Rust 1.95",
    detail: "CloudTrail → OCSF 1.3 normalizer; single static binary < 50 MB",
  },
  {
    icon: Workflow,
    label: "Redpanda (Kafka-compatible)",
    detail: "Topic: cloudtrail · partitioned by tenant_id · dual-listener setup",
  },
  {
    icon: Database,
    label: "RisingWave streaming SQL",
    detail: "entity_baselines + recent_events materialized views; Postgres wire",
  },
  {
    icon: Server,
    label: "attest-control-plane — axum 0.8",
    detail: "/healthz · /v1/events/recent · /v1/baselines/user/:name",
  },
  {
    icon: Lock,
    label: "OCSF 1.3 schema",
    detail: "Class 3002 Authentication · Class 6003 Cloud API Activity",
  },
  {
    icon: Globe,
    label: "Next.js 15 + Bun + shadcn",
    detail: "Workbench UI · Tailwind CSS · Geist Mono · deployed on Railway",
  },
];

export function SubstrateSection() {
  return (
    <section id="substrate" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Phase 1 — What&apos;s running
        </p>
        <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
          Streaming substrate, fully operational
        </h2>
        <p className="mt-4 max-w-3xl text-muted-foreground">
          Every component below is built, containerised, and passing end-to-end
          tests. CloudTrail events flow from collector to Redpanda to RisingWave
          to the control-plane API in under one second.
        </p>

        <motion.ul
          initial="hidden"
          whileInView="show"
          viewport={{ once: true, margin: "-60px" }}
          variants={{
            hidden: {},
            show: { transition: { staggerChildren: 0.05 } },
          }}
          className="mt-12 grid gap-3 sm:grid-cols-2 lg:grid-cols-3"
        >
          {stack.map((row) => (
            <motion.li
              key={row.label}
              variants={{
                hidden: { opacity: 0, y: 10 },
                show: { opacity: 1, y: 0 },
              }}
              className="flex gap-3 rounded-xl border border-border/70 bg-card/50 p-4"
            >
              <row.icon className="mt-0.5 h-5 w-5 shrink-0 text-primary" />
              <div>
                <p className="font-medium leading-tight">{row.label}</p>
                <p className="mt-1 text-sm text-muted-foreground">{row.detail}</p>
              </div>
            </motion.li>
          ))}
        </motion.ul>
      </div>
    </section>
  );
}
