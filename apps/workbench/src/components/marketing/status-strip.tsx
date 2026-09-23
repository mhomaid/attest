"use client";

import { motion } from "framer-motion";

const facts = [
  { value: "1", label: "live source", detail: "AWS CloudTrail" },
  { value: "10", label: "detections", detail: "AWS · Okta · M365 rules" },
  { value: "2", label: "triage paths", detail: "classifier · LLM" },
  { value: "5", label: "agent guards", detail: "policy + verifier" },
  { value: "v0", label: "status", detail: "Apache-2.0 · self-host" },
] as const;

export function StatusStrip() {
  return (
    <section aria-label="What ships today" className="border-b border-border/60 bg-card/20">
      <dl className="mx-auto grid max-w-6xl grid-cols-2 gap-px px-4 sm:grid-cols-3 sm:px-6 lg:grid-cols-5">
        {facts.map((f, i) => (
          <motion.div
            key={f.label}
            initial={{ opacity: 0, y: 8 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ delay: 0.06 * i, duration: 0.35 }}
            className="py-6"
          >
            <dt className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
              {f.label}
            </dt>
            <dd className="mt-1 text-2xl font-semibold tracking-tight">{f.value}</dd>
            <dd className="font-mono text-[10px] text-muted-foreground">{f.detail}</dd>
          </motion.div>
        ))}
      </dl>
    </section>
  );
}
