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
    <section id="substrate" className="border-b border-border/60 py-24">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        {/* Section header */}
        <div className="mb-14 flex flex-col items-start gap-3 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
              Phase 1 — What&apos;s running
            </p>
            <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl lg:text-5xl">
              Our Technology
            </h2>
          </div>
          <p className="max-w-sm text-sm text-muted-foreground sm:text-right">
            Every component is built, containerised, and passing end-to-end
            tests. Events flow collector → Redpanda → RisingWave → API in
            under&nbsp;one&nbsp;second.
          </p>
        </div>

        {/* Stack cards */}
        <motion.ul
          initial="hidden"
          whileInView="show"
          viewport={{ once: true, margin: "-60px" }}
          variants={{
            hidden: {},
            show: { transition: { staggerChildren: 0.06 } },
          }}
          className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3"
        >
          {stack.map((row) => (
            <motion.li
              key={row.label}
              variants={{
                hidden: { opacity: 0, y: 12 },
                show: { opacity: 1, y: 0, transition: { duration: 0.4, ease: [0.22, 1, 0.36, 1] } },
              }}
              className="group flex gap-3 rounded-xl border border-border/60 bg-card/40 p-5 backdrop-blur-sm transition-colors hover:border-primary/40 hover:bg-card/70"
            >
              <div className="mt-0.5 grid h-8 w-8 shrink-0 place-items-center rounded-lg border border-border/70 bg-secondary/60">
                <row.icon className="h-4 w-4 text-primary" />
              </div>
              <div>
                <p className="font-medium leading-snug">{row.label}</p>
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
