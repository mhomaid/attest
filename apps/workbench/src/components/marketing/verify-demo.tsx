"use client";

import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";

const steps = [
  { label: "Sign", out: "$ attest verify attestations.ndjson --key 4c1e…", tone: "idle" },
  { label: "Verify", out: "PASS  3/3 envelopes · chain intact", tone: "pass" },
  { label: "Tamper", out: "$ sed -i 's/benign/true_positive/' attestations.ndjson", tone: "idle" },
  { label: "Caught", out: "FAIL  a91c…  signature mismatch", tone: "fail" },
  { label: "Delete row", out: "$ sed -i '2d' attestations.ndjson", tone: "idle" },
  { label: "Caught", out: "FAIL  7be2…  chain break: prev_hash ≠ expected", tone: "fail" },
] as const;

const STEP_MS = 1900;

export function VerifyDemo() {
  const reduceMotion = useReducedMotion();
  const [phase, setPhase] = useState(1);
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    if (reduceMotion || paused) return;
    const id = window.setInterval(() => setPhase((p) => (p + 1) % steps.length), STEP_MS);
    return () => window.clearInterval(id);
  }, [reduceMotion, paused]);

  const tampered = phase === 2 || phase === 3;
  const deleted = phase === 4 || phase === 5;
  const step = steps[phase];

  return (
    <div
      className="grid gap-4 lg:grid-cols-2"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
    >
      <EnvelopeCard tampered={tampered} failed={phase === 3} />
      <ChainCard deleted={deleted} failed={phase === 5} />

      <div className="lg:col-span-2">
        <div className="flex flex-wrap gap-1.5">
          {steps.map((s, i) => (
            <button
              key={`${s.label}-${i}`}
              type="button"
              onClick={() => setPhase(i)}
              aria-current={phase === i ? "step" : undefined}
              className={cn(
                "rounded-full border px-3 py-1 font-mono text-[10px] uppercase tracking-wider transition-colors",
                phase === i
                  ? s.tone === "fail"
                    ? "border-red-500/50 bg-red-500/10 text-red-400"
                    : "border-primary/50 bg-primary/10 text-primary"
                  : "border-border/70 text-muted-foreground hover:border-border",
              )}
            >
              {s.label}
            </button>
          ))}
        </div>
        <div className="mt-3 rounded-xl border border-border/70 bg-black/40 px-4 py-3 font-mono text-xs">
          <AnimatePresence mode="wait">
            <motion.p
              key={phase}
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              aria-live="polite"
              className={cn(
                step.tone === "pass" && "text-signal-good",
                step.tone === "fail" && "text-red-400",
                step.tone === "idle" && "text-muted-foreground",
              )}
            >
              {step.out}
            </motion.p>
          </AnimatePresence>
        </div>
      </div>
    </div>
  );
}

function EnvelopeCard({ tampered, failed }: { tampered: boolean; failed: boolean }) {
  const rows = [
    ["agent_id", '"triager-hybrid-v1"'],
    ["verdict", tampered ? '"true_positive"' : '"benign"'],
    ["model_hash", '"8431…109f"'],
    ["prev_hash", '"e3b0…c442"'],
    ["signature", '"9f3a…77d1"'],
  ] as const;

  return (
    <div
      className={cn(
        "rounded-2xl border bg-card/50 p-5 transition-colors duration-300",
        failed ? "border-red-500/50" : "border-border/70",
      )}
    >
      <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
        Change one field
      </p>
      <pre className="mt-3 font-mono text-[12px] leading-6">
        <span className="text-muted-foreground">{"{"}</span>
        {rows.map(([k, v]) => {
          const hot = k === "verdict" && tampered;
          return (
            <motion.span
              key={k}
              className={cn("block pl-4", hot && "rounded bg-red-500/15 text-red-400")}
              animate={hot ? { x: [0, -3, 3, 0] } : { x: 0 }}
              transition={{ duration: 0.3 }}
            >
              <span className="text-signal-live">{k}</span>
              <span className="text-muted-foreground">: </span>
              <span className={hot ? "" : "text-foreground"}>{v}</span>
            </motion.span>
          );
        })}
        <span className="text-muted-foreground">{"}"}</span>
      </pre>
    </div>
  );
}

function ChainCard({ deleted, failed }: { deleted: boolean; failed: boolean }) {
  const blocks = [
    { id: "#41", hash: "e3b0" },
    { id: "#42", hash: "a91c" },
    { id: "#43", hash: "7be2" },
  ];
  const visible = deleted ? blocks.filter((b) => b.id !== "#42") : blocks;

  return (
    <div
      className={cn(
        "rounded-2xl border bg-card/50 p-5 transition-colors duration-300",
        failed ? "border-red-500/50" : "border-border/70",
      )}
    >
      <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
        Delete one row
      </p>
      <div className="mt-6 flex items-center justify-center gap-2">
        <AnimatePresence initial={false}>
          {visible.map((b, i) => {
            const broken = failed && b.id === "#43";
            return (
              <motion.div
                key={b.id}
                layout
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.6, y: 16 }}
                transition={{ duration: 0.3 }}
                className="flex items-center gap-2"
              >
                {i > 0 ? (
                  <span
                    aria-hidden
                    className={cn(
                      "h-px w-6 sm:w-10",
                      broken ? "bg-red-500/70" : "bg-primary/60",
                    )}
                  />
                ) : null}
                <div
                  className={cn(
                    "rounded-lg border px-3 py-2 text-center",
                    broken ? "border-red-500/60 bg-red-500/10" : "border-border/70 bg-background/50",
                  )}
                >
                  <p className="text-sm font-semibold">{b.id}</p>
                  <p className="font-mono text-[10px] text-muted-foreground">
                    {b.hash}…
                  </p>
                </div>
              </motion.div>
            );
          })}
        </AnimatePresence>
      </div>
      <p className="mt-6 text-center text-xs text-muted-foreground">
        Each envelope carries the hash of the one before it.
      </p>
    </div>
  );
}
