"use client";

import { motion } from "framer-motion";
import {
  Bot,
  Boxes,
  FileCheck2,
  GitBranch,
  Radar,
  Scale,
} from "lucide-react";

const container = {
  hidden: { opacity: 0 },
  show: {
    opacity: 1,
    transition: { staggerChildren: 0.06 },
  },
};

const item = {
  hidden: { opacity: 0, y: 16 },
  show: { opacity: 1, y: 0, transition: { duration: 0.4 } },
};

export function PillarsBento() {
  return (
    <section id="pillars" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Three first-of-kind capabilities
        </p>
        <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
          Built for the convergent product everyone is racing toward
        </h2>
        <p className="mt-4 max-w-3xl text-muted-foreground">
          Streaming SIEM, agentic SOC, AI-threat detection, and detection lifecycle, native from
          day one. Each pillar is described in the Attest blueprint (
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs">docs/</code>
          ).
        </p>

        <motion.div
          variants={container}
          initial="hidden"
          whileInView="show"
          viewport={{ once: true, margin: "-80px" }}
          className="mt-12 grid gap-4 md:grid-cols-3"
        >
          <motion.article
            variants={item}
            className="group relative overflow-hidden rounded-2xl border border-border bg-card/70 p-6 md:col-span-2 md:row-span-1"
          >
            <div className="absolute -right-10 -top-10 h-40 w-40 rounded-full bg-primary/10 blur-2xl transition-opacity group-hover:opacity-100" />
            <Radar className="h-8 w-8 text-primary" aria-hidden />
            <h3 className="mt-4 text-xl font-semibold">Agent-Aware Detection Fabric</h3>
            <p className="mt-3 text-sm leading-relaxed text-muted-foreground">
              Treat AI agents as identities: OpenTelemetry GenAI, MCP and A2A traces, tool calls,
              and reasoning logs correlated with identity, endpoint, and network telemetry. Answer{" "}
              <em className="text-foreground not-italic">
                “Is one of our AI agents being exploited or going rogue right now?”
              </em>
            </p>
          </motion.article>

          <motion.article
            variants={item}
            className="rounded-2xl border border-border bg-secondary/30 p-6"
          >
            <Scale className="h-7 w-7 text-signal-good" aria-hidden />
            <h3 className="mt-3 text-lg font-semibold">Verifiable Agentic SOC</h3>
            <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
              Specialized agents with signed traces, calibrated confidence, and deterministic
              shadow checks. Triager and Investigator run today; Hunter, Detection Engineer,
              and Responder are on the roadmap.
            </p>
          </motion.article>

          <motion.article
            variants={item}
            className="rounded-2xl border border-border bg-card/70 p-6 md:col-span-3"
          >
            <div className="flex flex-col gap-6 lg:flex-row lg:items-start lg:justify-between">
              <div className="max-w-xl">
                <Bot className="h-8 w-8 text-signal-live" aria-hidden />
                <h3 className="mt-3 text-xl font-semibold">Self-Improving Detection Mesh</h3>
                <p className="mt-3 text-sm leading-relaxed text-muted-foreground">
                  Map MITRE coverage, draft HELIQL from natural language, backtest on your
                  history, shadow-deploy, watch precision and drift, auto-tune thresholds, and
                  retire what no longer holds. Every change is a Git-backed PR humans approve.
                </p>
              </div>
              <ul className="grid shrink-0 gap-2 font-mono text-[11px] text-muted-foreground sm:grid-cols-2">
                {["NL → rule", "Backtest", "Shadow", "PR review", "Drift watch", "Auto-tune"].map(
                  (step) => (
                    <li
                      key={step}
                      className="flex items-center gap-2 rounded-md border border-border/60 bg-background/50 px-2 py-1.5"
                    >
                      <GitBranch className="h-3.5 w-3.5 text-primary" />
                      {step}
                    </li>
                  ),
                )}
              </ul>
            </div>
          </motion.article>
        </motion.div>
      </div>
    </section>
  );
}

export function FabricSection() {
  return (
    <section id="fabric" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <div className="flex flex-col gap-10 lg:flex-row lg:items-start">
          <div className="lg:w-2/5">
            <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
              Portable detection
            </p>
            <h2 className="mt-2 text-3xl font-semibold tracking-tight">One DSL, every horizon</h2>
            <p className="mt-4 text-muted-foreground">
              Write once in Attest&apos;s detection language: run in-stream on RisingWave / Arroyo,
              query warm history on ClickHouse over Iceberg, and federate to existing warehouses
              without rewriting your logic three times.
            </p>
          </div>
          <ul className="grid flex-1 gap-3 sm:grid-cols-2">
            {[
              {
                title: "Stream & hot path",
                body: "Streaming materialized views and alerts on Redpanda-backed pipelines.",
                icon: Boxes,
              },
              {
                title: "Warm & compliance",
                body: "OCSF-normalized Parquet on your bucket; auditors query the same rows.",
                icon: FileCheck2,
              },
              {
                title: "Federated",
                body: "Push-down to Snowflake, BigQuery, or Splunk during migration.",
                icon: Radar,
              },
              {
                title: "Open contracts",
                body: "OCSF, Sigma, MCP, OTEL GenAI. Compose and swap by design.",
                icon: GitBranch,
              },
            ].map((card) => (
              <li
                key={card.title}
                className="rounded-xl border border-border/80 bg-card/40 p-4 transition-colors hover:border-primary/30"
              >
                <card.icon className="h-5 w-5 text-primary" />
                <h3 className="mt-2 font-medium">{card.title}</h3>
                <p className="mt-1 text-sm text-muted-foreground">{card.body}</p>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}
