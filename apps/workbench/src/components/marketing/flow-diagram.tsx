"use client";

import { motion, useReducedMotion } from "framer-motion";
import { useCallback, useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

export type FlowStep = {
  label: string;
  hint?: string;
  detail?: string;
};

function useCycledIndex(length: number, cycleMs: number, enabled: boolean) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [inView, setInView] = useState(true);
  const [active, setActive] = useState(0);
  const [generation, setGeneration] = useState(0);

  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const io = new IntersectionObserver(
      ([entry]) => setInView(entry.isIntersecting),
      { threshold: 0.15 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  useEffect(() => {
    if (!enabled || !inView || length < 2) return;
    const id = window.setInterval(() => {
      setActive((i) => (i + 1) % length);
    }, cycleMs);
    return () => window.clearInterval(id);
  }, [length, cycleMs, enabled, generation, inView]);

  const select = useCallback((index: number) => {
    setActive(index);
    setGeneration((g) => g + 1);
  }, []);

  return { active, select, rootRef };
}

function Packet({ axis }: { axis: "x" | "y" }) {
  const horizontal = axis === "x";
  return (
    <motion.span
      aria-hidden
      className="pointer-events-none absolute h-1.5 w-1.5 rounded-full bg-primary"
      initial={
        horizontal
          ? { left: "0%", top: "50%", x: "-50%", y: "-50%", opacity: 0 }
          : { top: "0%", left: "50%", x: "-50%", y: "-50%", opacity: 0 }
      }
      animate={
        horizontal
          ? { left: ["0%", "100%"], opacity: [0, 1, 1, 0] }
          : { top: ["0%", "100%"], opacity: [0, 1, 1, 0] }
      }
      transition={{ duration: 1.4, repeat: Infinity, ease: "linear" }}
    />
  );
}

function NodeCard({
  step,
  index,
  active,
  size,
  onSelect,
}: {
  step: FlowStep;
  index: number;
  active: boolean;
  size: "sm" | "md";
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-current={active ? "step" : undefined}
      className={cn(
        "relative w-full rounded-xl border text-left transition-colors duration-300",
        size === "sm" ? "min-w-[6.5rem] px-3 py-2.5" : "min-w-[7.5rem] px-3.5 py-3",
        active
          ? "border-primary/45 bg-primary/8"
          : "border-border/70 bg-card/40 hover:border-border",
      )}
    >
      <p className="font-mono text-[10px] uppercase tracking-[0.16em] text-primary">
        {String(index + 1).padStart(2, "0")}
      </p>
      <p
        className={cn(
          "mt-1 font-semibold leading-snug",
          size === "sm" ? "text-[13px]" : "text-sm",
        )}
      >
        {step.label}
      </p>
      {step.hint ? (
        <p className="mt-0.5 font-mono text-[10px] leading-relaxed text-muted-foreground">
          {step.hint}
        </p>
      ) : null}
    </button>
  );
}

function Edge({
  active,
  orientation,
  motionOk,
}: {
  active: boolean;
  orientation: "x" | "y";
  motionOk: boolean;
}) {
  const horizontal = orientation === "x";
  return (
    <div
      aria-hidden
      className={cn(
        "relative shrink-0",
        horizontal
          ? "hidden w-7 self-center sm:w-9 md:flex md:h-4 md:items-center lg:w-12"
          : "mx-auto flex h-8 w-4 items-center justify-center md:hidden",
      )}
    >
      <span
        className={cn(
          "rounded-full transition-colors duration-300",
          horizontal ? "h-px w-full" : "h-full w-px",
          active ? "bg-primary/70" : "bg-border/80",
        )}
      />
      {active && motionOk ? <Packet axis={horizontal ? "x" : "y"} /> : null}
    </div>
  );
}

function Caption({ label, detail }: { label: string; detail: string }) {
  return (
    <p
      key={label}
      aria-live="polite"
      className="mt-4 text-sm leading-relaxed text-muted-foreground"
    >
      <span className="font-mono text-[10px] uppercase tracking-[0.16em] text-primary">
        {label}
      </span>
      <span className="mt-1 block">{detail}</span>
    </p>
  );
}

export function FlowRail({
  steps,
  cycleMs = 2200,
  size = "md",
  className,
}: {
  steps: readonly FlowStep[];
  cycleMs?: number;
  size?: "sm" | "md";
  className?: string;
}) {
  const reduceMotion = useReducedMotion();
  const { active, select, rootRef } = useCycledIndex(
    steps.length,
    cycleMs,
    reduceMotion !== true,
  );
  const current = steps[active];

  return (
    <div ref={rootRef} className={cn("w-full", className)}>
      <ol className="flex flex-col items-stretch md:flex-row md:items-center">
        {steps.map((step, i) => (
          <li
            key={step.label}
            className="flex flex-1 flex-col items-stretch md:flex-row md:items-center"
          >
            <NodeCard
              step={step}
              index={i}
              active={active === i}
              size={size}
              onSelect={() => select(i)}
            />
            {i < steps.length - 1 ? (
              <>
                <Edge
                  active={active === i}
                  orientation="y"
                  motionOk={reduceMotion !== true}
                />
                <Edge
                  active={active === i}
                  orientation="x"
                  motionOk={reduceMotion !== true}
                />
              </>
            ) : null}
          </li>
        ))}
      </ol>
      {current?.detail ? (
        <Caption label={current.label} detail={current.detail} />
      ) : null}
    </div>
  );
}

export function VerticalFlow({
  hops,
  cycleMs = 2400,
  className,
}: {
  hops: readonly { from: string; via: string; to: string }[];
  cycleMs?: number;
  className?: string;
}) {
  const reduceMotion = useReducedMotion();
  const { active, select, rootRef } = useCycledIndex(
    hops.length,
    cycleMs,
    reduceMotion !== true,
  );

  return (
    <div ref={rootRef} className={className}>
      <ol>
        {hops.map((hop, i) => {
          const on = active === i;
          return (
            <li key={`${hop.from}-${hop.to}`}>
              <button
                type="button"
                onClick={() => select(i)}
                aria-current={on ? "step" : undefined}
                className={cn(
                  "grid w-full gap-1 rounded-xl border px-4 py-3 text-left transition-colors duration-300 sm:grid-cols-[4.5rem_1fr]",
                  on
                    ? "border-primary/40 bg-primary/8"
                    : "border-border/60 bg-background/40 hover:border-border",
                )}
              >
                <p className="font-mono text-[10px] uppercase tracking-[0.16em] text-primary">
                  Hop {String(i + 1).padStart(2, "0")}
                </p>
                <div>
                  <p className="text-sm font-medium">{hop.from}</p>
                  <p className="mt-0.5 font-mono text-[11px] text-muted-foreground">
                    {hop.via}
                  </p>
                  <p className="mt-1 text-sm text-foreground">{hop.to}</p>
                </div>
              </button>
              {i < hops.length - 1 ? (
                <div
                  aria-hidden
                  className="relative mx-[1.35rem] h-7 w-px bg-border/70"
                >
                  <span
                    className={cn(
                      "absolute inset-0 bg-primary/70 transition-opacity duration-300",
                      on ? "opacity-100" : "opacity-0",
                    )}
                  />
                  {on && reduceMotion !== true ? <Packet axis="y" /> : null}
                </div>
              ) : null}
            </li>
          );
        })}
      </ol>
    </div>
  );
}

const forkPaths = {
  downLeft: "M50 2 L50 14 L16 46",
  downRight: "M50 2 L50 14 L84 46",
  joinLeft: "M16 2 L50 34 L50 46",
  joinRight: "M84 2 L50 34 L50 46",
} as const;

export function BranchFlow({
  start,
  left,
  right,
  join,
  end,
  cycleMs = 2000,
  className,
}: {
  start: FlowStep;
  left: FlowStep;
  right: FlowStep;
  join: FlowStep;
  end?: FlowStep;
  cycleMs?: number;
  className?: string;
}) {
  const reduceMotion = useReducedMotion();
  const nodes = end ? [start, left, right, join, end] : [start, left, right, join];
  const { active, select, rootRef } = useCycledIndex(
    nodes.length,
    cycleMs,
    reduceMotion !== true,
  );

  const viaLeft = active === 1;
  const viaRight = active === 2;
  const current = nodes[active];

  return (
    <div ref={rootRef} className={cn("w-full", className)}>
      <div className="mx-auto flex max-w-2xl flex-col items-center">
        <div className="w-full max-w-xs">
          <NodeCard
            step={start}
            index={0}
            active={active === 0}
            size="md"
            onSelect={() => select(0)}
          />
        </div>

        <svg
          viewBox="0 0 100 48"
          className="h-12 w-full max-w-md text-border"
          aria-hidden
        >
          <path
            d={forkPaths.downLeft}
            fill="none"
            stroke="currentColor"
            strokeWidth="0.8"
            className={cn(viaLeft && "text-primary flow-dash")}
          />
          <path
            d={forkPaths.downRight}
            fill="none"
            stroke="currentColor"
            strokeWidth="0.8"
            className={cn(viaRight && "text-primary flow-dash")}
          />
        </svg>

        <div className="grid w-full grid-cols-2 gap-3 sm:gap-6">
          <NodeCard
            step={left}
            index={1}
            active={active === 1}
            size="md"
            onSelect={() => select(1)}
          />
          <NodeCard
            step={right}
            index={2}
            active={active === 2}
            size="md"
            onSelect={() => select(2)}
          />
        </div>

        <svg
          viewBox="0 0 100 48"
          className="h-12 w-full max-w-md text-border"
          aria-hidden
        >
          <path
            d={forkPaths.joinLeft}
            fill="none"
            stroke="currentColor"
            strokeWidth="0.8"
            className={cn(viaLeft && "text-primary flow-dash")}
          />
          <path
            d={forkPaths.joinRight}
            fill="none"
            stroke="currentColor"
            strokeWidth="0.8"
            className={cn(viaRight && "text-primary flow-dash")}
          />
        </svg>

        <div className="w-full max-w-xs">
          <NodeCard
            step={join}
            index={3}
            active={active === 3}
            size="md"
            onSelect={() => select(3)}
          />
        </div>

        {end ? (
          <>
            <div aria-hidden className="relative mx-auto h-8 w-px bg-border/70">
              <span
                className={cn(
                  "absolute inset-0 bg-primary/70 transition-opacity duration-300",
                  active === 3 ? "opacity-100" : "opacity-0",
                )}
              />
              {active === 3 && reduceMotion !== true ? (
                <Packet axis="y" />
              ) : null}
            </div>
            <div className="w-full max-w-xs">
              <NodeCard
                step={end}
                index={4}
                active={active === 4}
                size="md"
                onSelect={() => select(4)}
              />
            </div>
          </>
        ) : null}
      </div>

      {current?.detail ? (
        <Caption label={current.label} detail={current.detail} />
      ) : null}
    </div>
  );
}
