"use client";

import { cn } from "@/lib/utils";

/** Soft green aurora (page grid is on `.marketing.sf-grid-page` in `globals.css`). */
export function GridBackground({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        "pointer-events-none absolute inset-0 overflow-hidden",
        className,
      )}
    >
      <div className="absolute -top-40 left-1/2 h-[42rem] w-[min(100%,80rem)] -translate-x-1/2 rounded-full bg-[radial-gradient(closest-side,oklch(0.45_0.14_145/0.4),transparent)] blur-3xl" />
      <div className="absolute bottom-0 right-0 h-80 w-80 rounded-full bg-[radial-gradient(closest-side,oklch(0.35_0.1_165/0.22),transparent)] blur-3xl" />
    </div>
  );
}
