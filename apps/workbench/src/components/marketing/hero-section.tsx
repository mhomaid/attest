"use client";

import { motion } from "framer-motion";
import { ArrowRight, GitBranch } from "lucide-react";
import { GridBackground } from "@/components/marketing/grid-background";
import { VerifyDemo } from "@/components/marketing/verify-demo";
import { analytics } from "@/lib/analytics";

const fadeUp = {
  hidden: { opacity: 0, y: 24 },
  visible: (i: number) => ({
    opacity: 1,
    y: 0,
    transition: { delay: 0.09 * i, duration: 0.55, ease: [0.22, 1, 0.36, 1] as const },
  }),
};

const proof = [
  "<5 ms classifier, P99 asserted in CI",
  "Ed25519 + hash-chained log",
  "Apache-2.0 · self-hostable",
] as const;

export function HeroSection() {
  return (
    <section className="relative overflow-hidden border-b border-border/60 pt-28 pb-20">
      <GridBackground />
      <div className="relative z-10 mx-auto max-w-6xl px-4 sm:px-6">
        <motion.p
          custom={0}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mb-6 inline-flex items-center gap-2 rounded-full border border-border/70 bg-card/40 px-3.5 py-1 font-mono text-[10px] uppercase tracking-[0.22em] text-muted-foreground backdrop-blur-sm"
        >
          <span className="h-1.5 w-1.5 rounded-full bg-signal-good shadow-[0_0_8px] shadow-signal-good/60" />
          Open-source verifiable SOC
        </motion.p>

        <motion.h1
          custom={1}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="max-w-4xl font-sans text-[clamp(2.25rem,5.5vw,4rem)] font-bold leading-[1.06] tracking-tight"
        >
          Every AI verdict is a{" "}
          <span className="bg-gradient-to-br from-primary via-lime-300/90 to-emerald-400/70 bg-clip-text text-transparent">
            signed object you can check.
          </span>
        </motion.h1>

        <motion.p
          custom={2}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-5 max-w-2xl text-[1.0625rem] leading-[1.7] text-muted-foreground"
        >
          Change one field or delete one row, and <code className="font-mono text-foreground">attest verify</code> fails.
        </motion.p>

        <motion.div
          custom={3}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-8 flex flex-wrap items-center gap-3"
        >
          <a
            href="#try"
            onClick={() => analytics.marketing_cta_clicked("hero_try_demo")}
            className="inline-flex items-center gap-2 rounded-md bg-primary px-5 py-2.5 text-sm font-semibold text-primary-foreground shadow-[0_0_20px_-4px] shadow-primary/50 transition-opacity hover:opacity-90"
          >
            Try a live verdict
            <ArrowRight className="h-4 w-4" />
          </a>
          <a
            href="#pipeline"
            onClick={() => analytics.marketing_cta_clicked("hero_how_it_works")}
            className="inline-flex items-center gap-2 rounded-md border border-border/80 bg-card/50 px-5 py-2.5 text-sm font-medium text-foreground backdrop-blur-sm transition-colors hover:bg-card"
          >
            See how it works
          </a>
          <a
            href="https://github.com/mhomaid/attest"
            target="_blank"
            rel="noreferrer"
            onClick={() => analytics.marketing_cta_clicked("hero_github")}
            className="inline-flex items-center gap-2 rounded-md border border-border/80 bg-card/50 px-5 py-2.5 text-sm font-medium text-foreground backdrop-blur-sm transition-colors hover:bg-card"
          >
            <GitBranch className="h-4 w-4" />
            View on GitHub
          </a>
        </motion.div>

        <motion.ul
          custom={4}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-6 flex flex-wrap gap-x-5 gap-y-1 font-mono text-[11px] text-muted-foreground"
        >
          {proof.map((p) => (
            <li key={p} className="flex items-center gap-2">
              <span className="h-1 w-1 rounded-full bg-primary" />
              {p}
            </li>
          ))}
        </motion.ul>

        <motion.div
          id="verify"
          custom={5}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-12 scroll-mt-20"
        >
          <VerifyDemo />
        </motion.div>
      </div>
    </section>
  );
}
