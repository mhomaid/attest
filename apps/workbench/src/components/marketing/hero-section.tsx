"use client";

import { motion } from "framer-motion";
import { ArrowRight, Fingerprint, Radio, ShieldCheck } from "lucide-react";
import Link from "next/link";
import { GridBackground } from "@/components/marketing/grid-background";

const fadeUp = {
  hidden: { opacity: 0, y: 20 },
  visible: (i: number) => ({
    opacity: 1,
    y: 0,
    transition: { delay: 0.08 * i, duration: 0.5, ease: [0.22, 1, 0.36, 1] as const },
  }),
};

export function HeroSection() {
  return (
    <section className="relative min-h-[min(100vh,56rem)] overflow-hidden border-b border-border/60 pt-24 pb-20">
      <GridBackground />
      <div className="relative z-10 mx-auto max-w-6xl px-4 sm:px-6">
        <motion.p
          custom={0}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mb-4 inline-flex items-center gap-2 rounded-full border border-border/80 bg-card/50 px-3 py-1 font-mono text-[10px] uppercase tracking-[0.2em] text-muted-foreground"
        >
          <span className="h-1.5 w-1.5 rounded-full bg-signal-good shadow-[0_0_8px] shadow-signal-good/60" />
          Verifiable agentic SIEM · Post-human threat landscape
        </motion.p>

        <motion.h1
          custom={1}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="max-w-4xl text-4xl font-semibold tracking-tight sm:text-5xl sm:leading-[1.08]"
        >
          The security platform where{" "}
          <span className="bg-gradient-to-r from-foreground via-primary to-lime-300/85 bg-clip-text text-transparent">
            AI agents are first-class
          </span>{" "}
          — on both sides of the attack.
        </motion.h1>

        <motion.p
          custom={2}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-5 max-w-2xl text-base leading-relaxed text-muted-foreground sm:text-lg"
        >
          Streaming-first operations, portable detections, and a multi-agent SOC that
          every auditor can trust: signed traces, calibrated confidence, and policy gates
          no model can bypass — built for regulated teams adopting AI at full speed.
        </motion.p>

        <motion.div
          custom={3}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-8 flex flex-wrap items-center gap-3"
        >
          <Link
            href="/workbench/queue"
            className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2.5 text-sm font-medium text-primary-foreground transition-opacity hover:opacity-95"
          >
            Explore the workbench
            <ArrowRight className="h-4 w-4" />
          </Link>
          <a
            href="#pillars"
            className="inline-flex items-center gap-2 rounded-md border border-border bg-card/60 px-4 py-2.5 text-sm font-medium text-foreground"
          >
            How Attest is different
          </a>
        </motion.div>

        <motion.dl
          custom={4}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-16 grid gap-4 sm:grid-cols-3"
        >
          {[
            {
              icon: Fingerprint,
              label: "Agent-Aware Detection Fabric",
              copy: "Detect AI agents, MCP/A2A abuse, and hybrid kill chains as first-class signal.",
            },
            {
              icon: ShieldCheck,
              label: "Verifiable Agentic SOC",
              copy: "Signed reasoning traces, calibration, and shadow checks for every verdict.",
            },
            {
              icon: Radio,
              label: "Self-Improving Detection Mesh",
              copy: "Propose, backtest, and ship detections in your DSL with human-in-the-loop PRs.",
            },
          ].map((item) => (
            <div
              key={item.label}
              className="rounded-xl border border-border/80 bg-card/50 p-4 shadow-sm backdrop-blur-sm"
            >
              <dt className="flex items-center gap-2 text-sm font-medium">
                <item.icon className="h-4 w-4 text-primary" />
                {item.label}
              </dt>
              <dd className="mt-2 text-sm leading-relaxed text-muted-foreground">{item.copy}</dd>
            </div>
          ))}
        </motion.dl>
      </div>
    </section>
  );
}
