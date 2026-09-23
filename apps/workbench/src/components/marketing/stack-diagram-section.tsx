"use client";

import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { ArrowRight, ShieldX } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { planes } from "@/components/marketing/stack-data";
import { analytics } from "@/lib/analytics";
import { cn } from "@/lib/utils";

const GATE_MS = 3600;
const DENY_AFTER_MS = 1400;

export function StackDiagramSection() {
  const reduceMotion = useReducedMotion();
  const [cycle, setCycle] = useState(0);
  const [paused, setPaused] = useState(false);
  const blocked = Boolean(reduceMotion) || cycle % 2 === 1;

  useEffect(() => {
    if (reduceMotion || paused) return;
    const delay = cycle % 2 === 0 ? DENY_AFTER_MS : GATE_MS - DENY_AFTER_MS;
    const id = window.setTimeout(() => setCycle((c) => c + 1), delay);
    return () => window.clearTimeout(id);
  }, [cycle, paused, reduceMotion]);

  return (
    <section id="stack" className="scroll-mt-16 border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Logical architecture
        </p>
        <h2 className="mt-3 max-w-3xl text-2xl font-semibold tracking-tight sm:text-3xl">
          Six planes. The agents never reach storage on their own.
        </h2>
        <p className="mt-3 max-w-2xl text-sm text-muted-foreground">
          One job per plane. The chip inventory lives on the stack page — this is the map.
        </p>

        <ol
          className="mt-10 divide-y divide-border/60 overflow-hidden rounded-2xl border border-border/70"
          onMouseEnter={() => setPaused(true)}
          onMouseLeave={() => setPaused(false)}
        >
          {planes.map((plane, i) => {
            const isAgentic = plane.id === "agentic";
            const isStorage = plane.id === "storage";
            return (
              <motion.li
                key={plane.id}
                initial={{ opacity: 0, y: 8 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true, margin: "-40px" }}
                transition={{ delay: 0.04 * i, duration: 0.3 }}
                className={cn(
                  "grid gap-1 bg-card/30 px-5 py-4 sm:grid-cols-[2.5rem_minmax(0,14rem)_1fr_auto] sm:items-baseline sm:gap-4",
                  isAgentic && blocked && "bg-red-500/[0.04]",
                  isStorage && blocked && "bg-red-500/[0.03]",
                )}
              >
                <span className="font-mono text-[11px] text-muted-foreground">
                  {String(i + 1).padStart(2, "0")}
                </span>
                <p className="text-sm font-semibold">{plane.name}</p>
                <p className="text-sm text-muted-foreground">{plane.summary}</p>
                <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
                  {plane.group === "data" ? "data" : "control"}
                </p>
                {isAgentic ? (
                  <div className="sm:col-span-4">
                    <GateCallout blocked={blocked} />
                  </div>
                ) : null}
              </motion.li>
            );
          })}
        </ol>

        <p className="mt-6">
          <Link
            href="/stack"
            onClick={() => analytics.marketing_cta_clicked("stack_full_page")}
            className="inline-flex items-center gap-1.5 text-sm font-medium text-primary hover:underline"
          >
            Every component, with repo paths
            <ArrowRight className="h-4 w-4" />
          </Link>
        </p>
      </div>
    </section>
  );
}

function GateCallout({ blocked }: { blocked: boolean }) {
  return (
    <div className="mt-2 flex flex-wrap items-center gap-2 font-mono text-[11px]">
      <AnimatePresence mode="wait">
        {blocked ? (
          <motion.p
            key="denied"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            className="flex items-center gap-2 text-red-400"
            aria-live="polite"
          >
            <ShieldX className="h-3.5 w-3.5" />
            DENIED agentic → storage (direct)
          </motion.p>
        ) : (
          <motion.p
            key="attempt"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            className="text-muted-foreground"
          >
            agent tries storage.query()
          </motion.p>
        )}
      </AnimatePresence>
      <span className="text-muted-foreground">·</span>
      <span className={cn(blocked ? "text-signal-good" : "text-muted-foreground")}>
        ALLOW via MCP + policy
      </span>
    </div>
  );
}
