import { Bot, CheckCircle2, Loader2, TriangleAlert } from "lucide-react";
import type { ElementType } from "react";
import type { AgentStatus } from "@/lib/mock-data";
import { cn } from "@/lib/utils";

const statusConfig = {
  idle: {
    icon: Bot,
    className: "border-border bg-secondary text-muted-foreground",
  },
  running: {
    icon: Loader2,
    className: "border-signal-live/40 bg-signal-live/10 text-signal-live",
  },
  complete: {
    icon: CheckCircle2,
    className: "border-signal-good/40 bg-signal-good/10 text-signal-good",
  },
  degraded: {
    icon: TriangleAlert,
    className: "border-severity-medium/40 bg-severity-medium/10 text-severity-medium",
  },
} satisfies Record<AgentStatus["status"], { icon: ElementType; className: string }>;

export function AgentLiveStatusPill({ status }: { status: AgentStatus }) {
  const config = statusConfig[status.status];
  const Icon = config.icon;

  return (
    <div
      className={cn(
        "inline-flex max-w-full items-center gap-1.5 rounded-md border px-1.5 py-1 text-[11px]",
        config.className,
      )}
      title={`${status.agent}: ${status.step} (${status.toolsUsed} tools used)`}
    >
      <Icon className={cn("h-3.5 w-3.5", status.status === "running" && "animate-spin")} />
      <span className="font-medium">{status.agent}</span>
      <span className="truncate text-muted-foreground">{status.step}</span>
    </div>
  );
}
