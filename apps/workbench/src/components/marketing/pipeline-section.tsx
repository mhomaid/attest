"use client";

import { motion } from "framer-motion";
import { PipelineDiagram } from "@/components/marketing/pipeline-diagram";

export function PipelineSection() {
  return (
    <section id="pipeline" className="scroll-mt-16 border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.45 }}
        >
          <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
            One alert, end to end
          </p>
          <h2 className="mt-3 max-w-3xl text-2xl font-semibold tracking-tight sm:text-3xl">
            Known patterns take the fast lane. Only novel ones reach the LLM.
          </h2>
          <div className="mt-10 rounded-2xl border border-border/70 bg-card/30 p-4 sm:p-6">
            <PipelineDiagram />
          </div>
          <div className="mt-4 flex flex-wrap gap-x-6 gap-y-1 font-mono text-[11px] text-muted-foreground">
            <span className="flex items-center gap-2">
              <span className="h-2 w-2 rounded-full bg-signal-good" /> classifier path
            </span>
            <span className="flex items-center gap-2">
              <span className="h-2 w-2 rounded-full bg-signal-live" /> LLM path, tools gated by policy
            </span>
            <span className="flex items-center gap-2">
              <span className="h-2 w-2 rounded-full bg-primary" /> both end signed
            </span>
          </div>
        </motion.div>
      </div>
    </section>
  );
}
