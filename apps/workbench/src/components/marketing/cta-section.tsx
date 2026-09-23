"use client";

import { motion } from "framer-motion";
import { ArrowRight, Star } from "lucide-react";
import { analytics } from "@/lib/analytics";

export function CtaSection() {
  return (
    <section className="py-20">
      <motion.div
        initial={{ opacity: 0, y: 24 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        transition={{ duration: 0.45 }}
        className="mx-auto max-w-6xl px-4 sm:px-6"
      >
        <div className="relative overflow-hidden rounded-2xl border border-primary/30 bg-gradient-to-br from-primary/15 via-card to-card px-6 py-12 sm:px-10 sm:py-14">
          <div className="absolute -right-20 -top-20 h-64 w-64 rounded-full bg-primary/20 blur-3xl" />
          <div className="relative z-10 max-w-2xl">
            <h2 className="text-2xl font-semibold tracking-tight sm:text-3xl">
              Make your AI SOC show its work
            </h2>
            <p className="mt-3 text-muted-foreground">
              Clone it, break the log, watch verify catch it. Testers and focused PRs are welcome —
              open an issue first if the change is larger than a bug fix.
            </p>
            <div className="mt-6 flex flex-wrap gap-3">
              <a
                href="/waitlist"
                onClick={() => analytics.marketing_cta_clicked("cta_waitlist")}
                className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2.5 text-sm font-medium text-primary-foreground"
              >
                Request access
                <ArrowRight className="h-4 w-4" />
              </a>
              <a
                href="#try"
                onClick={() => analytics.marketing_cta_clicked("cta_try_demo")}
                className="inline-flex items-center gap-2 rounded-md border border-border bg-background/60 px-4 py-2.5 text-sm font-medium"
              >
                Try a live verdict
              </a>
              <a
                href="https://github.com/mhomaid/attest/issues/new"
                target="_blank"
                rel="noreferrer"
                onClick={() => analytics.marketing_cta_clicked("cta_open_issue")}
                className="inline-flex items-center gap-2 rounded-md border border-border bg-background/60 px-4 py-2.5 text-sm font-medium"
              >
                <Star className="h-4 w-4" />
                Open an issue or PR
              </a>
            </div>
          </div>
        </div>
      </motion.div>
    </section>
  );
}
