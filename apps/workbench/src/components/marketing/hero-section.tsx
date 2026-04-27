"use client";

import { motion } from "framer-motion";
import { ArrowRight, Fingerprint, Radio, ShieldCheck } from "lucide-react";
import Link from "next/link";
import { GridBackground } from "@/components/marketing/grid-background";

const fadeUp = {
  hidden: { opacity: 0, y: 24 },
  visible: (i: number) => ({
    opacity: 1,
    y: 0,
    transition: { delay: 0.09 * i, duration: 0.55, ease: [0.22, 1, 0.36, 1] as const },
  }),
};

export function HeroSection() {
  return (
    <section className="relative min-h-[min(100vh,60rem)] overflow-hidden border-b border-border/60 pt-28 pb-24">
      <GridBackground />
      <div className="relative z-10 mx-auto max-w-5xl px-4 sm:px-6">

        {/* Eyebrow badge */}
        <motion.p
          custom={0}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mb-6 inline-flex items-center gap-2 rounded-full border border-border/70 bg-card/40 px-3.5 py-1 font-mono text-[10px] uppercase tracking-[0.22em] text-muted-foreground backdrop-blur-sm"
        >
          <span className="h-1.5 w-1.5 rounded-full bg-signal-good shadow-[0_0_8px] shadow-signal-good/60" />
          Verifiable agentic SIEM · Post-human threat landscape
        </motion.p>

        {/* Headline */}
        <motion.h1
          custom={1}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="font-sans text-[clamp(2.25rem,5.5vw,4rem)] font-bold leading-[1.06] tracking-tight"
        >
          The security platform where{" "}
          <span className="bg-gradient-to-br from-primary via-lime-300/90 to-emerald-400/70 bg-clip-text text-transparent">
            AI agents are first-class
          </span>
          {" "}— on both sides of the attack.
        </motion.h1>

        {/* Sub-copy */}
        <motion.p
          custom={2}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-6 max-w-2xl text-[1.0625rem] leading-[1.75] text-muted-foreground"
        >
          Streaming-first operations, portable detections, and a multi-agent SOC
          that every auditor can trust: signed traces, calibrated confidence, and
          policy gates no model can bypass — built for regulated teams adopting AI
          at full speed.
        </motion.p>

        {/* CTAs */}
        <motion.div
          custom={3}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-9 flex flex-wrap items-center gap-3"
        >
          <Link
            href="/workbench/queue"
            className="inline-flex items-center gap-2 rounded-md bg-primary px-5 py-2.5 text-sm font-semibold text-primary-foreground shadow-[0_0_20px_-4px] shadow-primary/50 transition-opacity hover:opacity-90"
          >
            Explore the workbench
            <ArrowRight className="h-4 w-4" />
          </Link>
          <a
            href="#pillars"
            className="inline-flex items-center gap-2 rounded-md border border-border/80 bg-card/50 px-5 py-2.5 text-sm font-medium text-foreground backdrop-blur-sm transition-colors hover:bg-card"
          >
            How Attest is different
          </a>
        </motion.div>

        {/* Feature cards */}
        <motion.dl
          custom={4}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-18 grid gap-3 sm:grid-cols-3"
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
              className="rounded-xl border border-border/70 bg-card/40 p-5 shadow-sm backdrop-blur-sm"
            >
              <dt className="flex items-center gap-2 text-sm font-semibold">
                <item.icon className="h-4 w-4 shrink-0 text-primary" />
                {item.label}
              </dt>
              <dd className="mt-2 text-sm leading-relaxed text-muted-foreground">
                {item.copy}
              </dd>
            </div>
          ))}
        </motion.dl>
      </div>
    </section>
  );
}
