import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

type StatusTone = "critical" | "high" | "medium" | "low" | "info" | "muted" | "good";

const toneClasses: Record<StatusTone, string> = {
  critical: "border-severity-critical/40 bg-severity-critical/10 text-severity-critical",
  high: "border-severity-high/40 bg-severity-high/10 text-severity-high",
  medium: "border-severity-medium/40 bg-severity-medium/10 text-severity-medium",
  low: "border-severity-low/40 bg-severity-low/10 text-severity-low",
  info: "border-signal-live/40 bg-signal-live/10 text-signal-live",
  muted: "border-border bg-secondary text-muted-foreground",
  good: "border-signal-good/40 bg-signal-good/10 text-signal-good",
};

export function StatusBadge({
  children,
  tone = "muted",
  className,
}: {
  children: ReactNode;
  tone?: StatusTone;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md border px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-[0.18em]",
        toneClasses[tone],
        className,
      )}
    >
      {children}
    </span>
  );
}
