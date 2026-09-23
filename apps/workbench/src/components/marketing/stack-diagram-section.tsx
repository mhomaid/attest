"use client";

import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { ArrowRight, ShieldX } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import {
  isPlanned,
  planes,
  repoLink,
  type PlaneComponent,
} from "@/components/marketing/stack-data";
import { analytics } from "@/lib/analytics";
import { cn } from "@/lib/utils";

const GATE_MS = 3600;
const DENY_AFTER_MS = 1400;

export function StackDiagramSection() {
  const reduceMotion = useReducedMotion();
  const [hover, setHover] = useState<{ plane: string; role: string } | null>(null);
  const [cycle, setCycle] = useState(0);
  const [paused, setPaused] = useState(false);
  const blocked = Boolean(reduceMotion) || cycle % 2 === 1;
  const total = planes.reduce((n, p) => n + p.components.length, 0);

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
        <div className="mt-3 flex flex-wrap items-end justify-between gap-4">
          <h2 className="max-w-3xl text-2xl font-semibold tracking-tight sm:text-3xl">
            Six planes. The agents never reach storage on their own.
          </h2>
          <Link
            href="/stack"
            onClick={() => analytics.marketing_cta_clicked("stack_full_page")}
            className="inline-flex items-center gap-1.5 text-sm font-medium text-primary hover:underline"
          >
            Every component in detail
            <ArrowRight className="h-4 w-4" />
          </Link>
        </div>
        <p className="mt-3 max-w-2xl text-sm text-muted-foreground">
          {total} components, one repo. Same six-plane model as the architecture docs.
        </p>

        <ol
          className="mt-10 space-y-2"
          onMouseEnter={() => setPaused(true)}
          onMouseLeave={() => setPaused(false)}
        >
          {planes.map((plane, i) => {
            const isAgentic = plane.id === "agentic";
            const isStorage = plane.id === "storage";
            return (
              <motion.li
                key={plane.id}
                initial={{ opacity: 0, y: 10 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true, margin: "-40px" }}
                transition={{ delay: 0.04 * i, duration: 0.35 }}
                className={cn(
                  "rounded-2xl border bg-card/30 px-4 py-3 sm:px-5",
                  isAgentic && blocked && "border-red-500/35",
                  isStorage && blocked && "border-red-500/25",
                  !((isAgentic || isStorage) && blocked) && "border-border/70",
                )}
              >
                <div className="flex flex-wrap items-baseline justify-between gap-2">
                  <p className="text-sm font-semibold">
                    <span className="mr-2 font-mono text-[10px] text-muted-foreground">
                      {String(i + 1).padStart(2, "0")}
                    </span>
                    {plane.name}
                  </p>
                  <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
                    {plane.group === "data" ? "data plane" : "control plane"}
                  </p>
                </div>
                <ul className="mt-2.5 flex flex-wrap gap-1.5">
                  {plane.components.map((c) => (
                    <li key={c.name}>
                      <PlaneChip
                        component={c}
                        onHover={(role) => setHover({ plane: plane.id, role })}
                        onLeave={() => setHover(null)}
                      />
                    </li>
                  ))}
                </ul>
                <p
                  className={cn(
                    "mt-2 text-xs",
                    hover?.plane === plane.id ? "text-foreground" : "text-muted-foreground",
                  )}
                >
                  {hover?.plane === plane.id ? hover.role : plane.summary}
                </p>
                {isAgentic ? <GateCallout blocked={blocked} /> : null}
              </motion.li>
            );
          })}
        </ol>
      </div>
    </section>
  );
}

function GateCallout({ blocked }: { blocked: boolean }) {
  return (
    <div className="mt-3 flex flex-wrap items-center gap-2 rounded-xl border border-border/60 bg-background/50 px-3 py-2 font-mono text-[11px]">
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
            DENIED  agentic → storage (direct)
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
        ALLOW  via MCP + policy → detection / governed query
      </span>
    </div>
  );
}

export function PlaneChip({
  component,
  onHover,
  onLeave,
}: {
  component: PlaneComponent;
  onHover?: (role: string) => void;
  onLeave?: () => void;
}) {
  const planned = isPlanned(component);
  const className = cn(
    "inline-flex items-baseline gap-1.5 rounded-md border px-2.5 py-1 text-xs transition-colors",
    planned
      ? "border-dashed border-border/80 bg-transparent text-muted-foreground"
      : "border-border/70 bg-background/60 hover:border-primary/60 hover:bg-primary/10",
  );
  const body = (
    <>
      <span className="font-semibold">{component.name}</span>
      <span className="font-mono text-[10px] text-muted-foreground">{component.tech}</span>
      {planned ? (
        <span className="rounded-sm border border-border/60 px-1 py-px font-mono text-[8px] uppercase tracking-wider">
          planned
        </span>
      ) : null}
    </>
  );
  const handlers = {
    onMouseEnter: () => onHover?.(component.role),
    onMouseLeave: onLeave,
    onFocus: () => onHover?.(component.role),
    onBlur: onLeave,
    title: component.role,
  };

  if (!component.path) {
    return (
      <span className={className} {...handlers}>
        {body}
      </span>
    );
  }

  return (
    <a
      href={repoLink(component.path)}
      target="_blank"
      rel="noreferrer"
      className={className}
      {...handlers}
    >
      {body}
    </a>
  );
}
