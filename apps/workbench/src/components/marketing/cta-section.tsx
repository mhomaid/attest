"use client";

import { motion } from "framer-motion";
import { ArrowRight } from "lucide-react";
import Link from "next/link";

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
              See the analyst console
            </h2>
            <p className="mt-3 text-muted-foreground">
              The workbench is a dense SOC surface: live queue, case investigation, attested
              reasoning, and MITRE-oriented coverage. Start with the interactive shell while the
              platform lands.
            </p>
            <div className="mt-6 flex flex-wrap gap-3">
              <Link
                href="/login"
                className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2.5 text-sm font-medium text-primary-foreground"
              >
                Sign in
                <ArrowRight className="h-4 w-4" />
              </Link>
              <a
                href="#architecture"
                className="inline-flex items-center rounded-md border border-border bg-background/60 px-4 py-2.5 text-sm font-medium"
              >
                Read the architecture
              </a>
            </div>
          </div>
        </div>
      </motion.div>
    </section>
  );
}
