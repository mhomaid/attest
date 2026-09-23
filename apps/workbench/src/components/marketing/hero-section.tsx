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
      <div className="relative z-10 mx-auto max-w-6xl px-4 text-center sm:px-6">
        <motion.div
          custom={0}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mb-6 flex justify-center"
        >
          <p className="inline-flex items-center gap-2 rounded-full border border-border/70 bg-card/40 px-3.5 py-1 font-mono text-[10px] uppercase tracking-[0.22em] text-muted-foreground backdrop-blur-sm">
            <span className="h-1.5 w-1.5 rounded-full bg-signal-good shadow-[0_0_8px] shadow-signal-good/60" />
            The problem we built Attest for
          </p>
        </motion.div>

        <motion.h1
          custom={1}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mx-auto max-w-4xl font-sans text-[clamp(2.25rem,5.5vw,4rem)] font-bold leading-[1.06] tracking-tight"
        >
          AI SOCs ask you to trust the verdict.{" "}
          <span className="bg-gradient-to-br from-primary via-lime-300/90 to-emerald-400/70 bg-clip-text text-transparent">
            We made it something you can check.
          </span>
        </motion.h1>

        <motion.p
          custom={2}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mx-auto mt-5 max-w-2xl text-[1.0625rem] leading-[1.7] text-muted-foreground"
        >
          When an agent closes an alert or isolates a host, most tools leave a chat transcript.
          An auditor cannot replay it. A swapped model or a deleted row leaves no mark. We built
          Attest so every decision is an Ed25519-signed envelope on a hash chain —{" "}
          <code className="font-mono text-foreground">attest verify</code> fails if anyone
          tampers.
        </motion.p>

        <motion.dl
          custom={3}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mx-auto mt-8 grid max-w-3xl gap-4 text-left sm:grid-cols-3"
        >
          {[
            {
              k: "Problem",
              v: "You cannot prove the verdict is the one the agent produced, or that the model was the one you approved.",
            },
            {
              k: "Why we built it",
              v: "A CISO still has to defend the action. Trusting the vendor's UI is not an audit trail.",
            },
            {
              k: "How",
              v: "Sign the envelope, chain it to the row before it, verify offline. Same CLI we run in CI.",
            },
          ].map((item) => (
            <div
              key={item.k}
              className="rounded-xl border border-border/70 bg-card/40 px-4 py-3 backdrop-blur-sm"
            >
              <dt className="font-mono text-[10px] uppercase tracking-[0.18em] text-primary">
                {item.k}
              </dt>
              <dd className="mt-1.5 text-sm leading-relaxed text-muted-foreground">{item.v}</dd>
            </div>
          ))}
        </motion.dl>

        <motion.div
          custom={4}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-8 flex flex-wrap items-center justify-center gap-3"
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
            href="/waitlist"
            onClick={() => analytics.marketing_cta_clicked("hero_waitlist")}
            className="inline-flex items-center gap-2 rounded-md border border-border/80 bg-card/50 px-5 py-2.5 text-sm font-medium text-foreground backdrop-blur-sm transition-colors hover:bg-card"
          >
            Request access
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
          custom={5}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mt-6 flex flex-wrap justify-center gap-x-5 gap-y-1 font-mono text-[11px] text-muted-foreground"
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
          custom={6}
          initial="hidden"
          animate="visible"
          variants={fadeUp}
          className="mx-auto mt-12 max-w-4xl scroll-mt-20 text-left"
        >
          <VerifyDemo />
        </motion.div>
      </div>
    </section>
  );
}
