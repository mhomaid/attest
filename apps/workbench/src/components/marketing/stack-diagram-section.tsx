"use client";

import { motion, useReducedMotion } from "framer-motion";
import { ArrowRight } from "lucide-react";
import Link from "next/link";
import { useState } from "react";
import { repoLink, stackLayers } from "@/components/marketing/stack-data";
import { analytics } from "@/lib/analytics";
import { cn } from "@/lib/utils";

export function StackDiagramSection() {
  const reduceMotion = useReducedMotion();
  const [hover, setHover] = useState<{ layer: string; role: string } | null>(null);
  const total = stackLayers.reduce((n, l) => n + l.components.length, 0);

  return (
    <section id="stack" className="scroll-mt-16 border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          What it&apos;s built from
        </p>
        <div className="mt-3 flex flex-wrap items-end justify-between gap-4">
          <h2 className="max-w-3xl text-2xl font-semibold tracking-tight sm:text-3xl">
            {stackLayers.length} layers, {total} components, one repo.
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

        <div className="relative mt-10">
          <div aria-hidden className="absolute top-2 bottom-2 left-[0.6875rem] w-px bg-border sm:left-[0.9375rem]" />
          {!reduceMotion ? (
            <motion.div
              aria-hidden
              className="absolute left-[0.5rem] h-2 w-2 rounded-full bg-primary shadow-[0_0_10px] shadow-primary sm:left-[0.75rem]"
              animate={{ top: ["0%", "100%"] }}
              transition={{ duration: 6, repeat: Infinity, ease: "linear" }}
            />
          ) : null}

          <ol className="space-y-2">
            {stackLayers.map((layer, i) => (
              <motion.li
                key={layer.id}
                initial={{ opacity: 0, x: -12 }}
                whileInView={{ opacity: 1, x: 0 }}
                viewport={{ once: true, margin: "-40px" }}
                transition={{ delay: 0.04 * i, duration: 0.35 }}
                className="relative grid gap-3 pl-8 sm:grid-cols-[11rem_minmax(0,1fr)] sm:pl-10"
              >
                <span
                  aria-hidden
                  className="absolute top-3.5 left-[0.4375rem] h-2.5 w-2.5 rounded-full border border-primary/60 bg-background sm:left-[0.6875rem]"
                />
                <div className="pt-2">
                  <p className="text-sm font-semibold">
                    <span className="mr-2 font-mono text-[10px] text-muted-foreground">
                      {String(i + 1).padStart(2, "0")}
                    </span>
                    {layer.name}
                  </p>
                </div>
                <div className="rounded-xl border border-border/70 bg-card/30 px-3 py-2.5">
                  <ul className="flex flex-wrap gap-1.5">
                    {layer.components.map((c) => (
                      <li key={c.name}>
                        <a
                          href={repoLink(c.path)}
                          target="_blank"
                          rel="noreferrer"
                          title={c.role}
                          onMouseEnter={() => setHover({ layer: layer.id, role: c.role })}
                          onMouseLeave={() => setHover(null)}
                          onFocus={() => setHover({ layer: layer.id, role: c.role })}
                          onBlur={() => setHover(null)}
                          className="inline-flex items-baseline gap-1.5 rounded-md border border-border/70 bg-background/60 px-2.5 py-1 text-xs transition-colors hover:border-primary/60 hover:bg-primary/10"
                        >
                          <span className="font-semibold">{c.name}</span>
                          <span className="font-mono text-[10px] text-muted-foreground">{c.tech}</span>
                        </a>
                      </li>
                    ))}
                  </ul>
                  <p
                    className={cn(
                      "mt-2 text-xs",
                      hover?.layer === layer.id ? "text-foreground" : "text-muted-foreground",
                    )}
                  >
                    {hover?.layer === layer.id ? hover.role : layer.summary}
                  </p>
                </div>
              </motion.li>
            ))}
          </ol>
        </div>
      </div>
    </section>
  );
}
