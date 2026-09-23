"use client";

import { motion } from "framer-motion";
import { Check, X } from "lucide-react";

const rows = [
  {
    topic: "The verdict record",
    blackBox: "A row in the vendor's database",
    attest: "Ed25519-signed envelope, hash-chained",
  },
  {
    topic: "Someone edits it",
    blackBox: "Nothing notices",
    attest: "attest verify fails on that row",
  },
  {
    topic: "Re-checking a decision",
    blackBox: "Ask the model again, get a new answer",
    attest: "Classifier re-run to the recorded score",
  },
  {
    topic: "Agent tool access",
    blackBox: "Rules live in the prompt",
    attest: "Policy engine outside the model",
  },
  {
    topic: "Where it runs",
    blackBox: "Their cloud",
    attest: "Your infrastructure, Apache-2.0",
  },
] as const;

export function CompareSection() {
  return (
    <section id="compare" className="scroll-mt-16 border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Why it matters
        </p>
        <h2 className="mt-3 max-w-3xl text-2xl font-semibold tracking-tight sm:text-3xl">
          Most AI SOCs ask you to trust the verdict. Attest lets you check it.
        </h2>

        <div className="mt-10 overflow-hidden rounded-2xl border border-border/70">
          <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] bg-card/40 font-mono text-[10px] uppercase tracking-[0.18em] sm:grid-cols-[12rem_minmax(0,1fr)_minmax(0,1fr)]">
            <span className="hidden px-4 py-3 sm:block" />
            <span className="px-4 py-3 text-muted-foreground">Black-box AI verdict</span>
            <span className="px-4 py-3 text-primary">Attest verdict</span>
          </div>
          {rows.map((r, i) => (
            <motion.div
              key={r.topic}
              initial={{ opacity: 0, x: -8 }}
              whileInView={{ opacity: 1, x: 0 }}
              viewport={{ once: true }}
              transition={{ delay: 0.07 * i, duration: 0.35 }}
              className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] border-t border-border/60 text-sm sm:grid-cols-[12rem_minmax(0,1fr)_minmax(0,1fr)]"
            >
              <span className="col-span-2 px-4 pt-3 font-semibold sm:col-span-1 sm:py-3">
                {r.topic}
              </span>
              <span className="flex items-start gap-2 px-4 py-3 text-muted-foreground">
                <X className="mt-0.5 h-4 w-4 shrink-0 text-red-400/80" />
                {r.blackBox}
              </span>
              <span className="flex items-start gap-2 px-4 py-3">
                <Check className="mt-0.5 h-4 w-4 shrink-0 text-signal-good" />
                {r.attest}
              </span>
            </motion.div>
          ))}
        </div>
      </div>
    </section>
  );
}
