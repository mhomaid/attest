"use client";

import { motion } from "framer-motion";
import { Cpu, Database, Globe, Lock, Server, Workflow } from "lucide-react";

const stack = [
  { icon: Workflow, label: "Redpanda + stream runtime", detail: "Kafka-compatible spine" },
  { icon: Database, label: "Apache Iceberg + OCSF", detail: "Customer-owned lakehouse" },
  { icon: Cpu, label: "Rust hot path", detail: "Collectors, orchestrator, policy" },
  { icon: Server, label: "ClickHouse analytics", detail: "Warm query + Iceberg reads" },
  { icon: Globe, label: "BYOC & air-gapped", detail: "Regulated deployment patterns" },
  { icon: Lock, label: "Attestation & policy", detail: "Ed25519 envelopes, PACl" },
];

export function SubstrateSection() {
  return (
    <section id="substrate" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Architectural foundations
        </p>
        <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
          Streaming-first, composable, open by contract
        </h2>
        <p className="mt-4 max-w-3xl text-muted-foreground">
          The same substrate patterns the market has proven — collection that shapes data at the
          edge, portable detections, and polyglot runtimes — with Attest&apos;s agent and
          attestation layer on top.
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
