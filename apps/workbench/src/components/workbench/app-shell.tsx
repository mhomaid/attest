import {
  Activity,
  Bell,
  Bot,
  Command,
  Crosshair,
  FileCode2,
  Gauge,
  RadioTower,
  Search,
  Settings,
  ShieldCheck,
  Siren,
  Users,
} from "lucide-react";
import Link from "next/link";
import type { ReactNode } from "react";
import { StatusBadge } from "@/components/workbench/status-badge";
import { cn } from "@/lib/utils";

const navItems = [
  { href: "/workbench/queue", label: "Queue", icon: Siren, count: "40" },
  { href: "/workbench/cases/case-1042", label: "Cases", icon: ShieldCheck },
  { href: "/workbench/hunt", label: "Hunt", icon: Crosshair },
  { href: "/workbench/detections", label: "Detections", icon: FileCode2 },
  { href: "/workbench/coverage", label: "Coverage", icon: Gauge },
  { href: "/workbench/agents", label: "Agents", icon: Bot, count: "6" },
  { href: "/admin", label: "Admin", icon: Users },
  { href: "/settings", label: "Settings", icon: Settings },
];

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="min-h-screen overflow-hidden bg-background text-foreground">
      <div className="flex min-h-screen">
        <aside className="hidden w-60 shrink-0 border-r border-border/80 bg-card/70 backdrop-blur xl:block">
          <div className="flex h-14 items-center gap-2 border-b border-border px-3">
            <div className="grid h-8 w-8 place-items-center rounded-md border border-primary/30 bg-primary/15 text-primary">
              <Activity className="h-4 w-4" />
            </div>
            <div>
              <div className="font-semibold leading-none tracking-tight">Attest</div>
              <div className="mt-1 font-mono text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                Analyst Workbench
              </div>
            </div>
          </div>

          <nav className="space-y-1 p-2">
            {navItems.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className={cn(
                  "group flex items-center justify-between rounded-md px-2 py-2 text-sm text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground",
                  item.href === "/workbench/queue" && "bg-secondary text-foreground",
                )}
              >
                <span className="flex items-center gap-2">
                  <item.icon className="h-4 w-4" />
                  {item.label}
                </span>
                {item.count ? (
                  <span className="rounded bg-background px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                    {item.count}
                  </span>
                ) : null}
              </Link>
            ))}
          </nav>
        </aside>

        <div className="flex min-w-0 flex-1 flex-col">
          <header className="flex h-14 shrink-0 items-center justify-between border-b border-border/80 bg-card/60 px-3 backdrop-blur">
            <div className="flex items-center gap-2">
              <StatusBadge tone="info">Live</StatusBadge>
              <StatusBadge tone="good">Classifier path online</StatusBadge>
              <StatusBadge tone="medium">Calibration 6d old</StatusBadge>
            </div>

            <div className="flex min-w-0 flex-1 justify-center px-4">
              <button className="flex h-8 w-full max-w-xl items-center justify-between rounded-md border border-border bg-background/80 px-2 text-left text-xs text-muted-foreground shadow-sm">
                <span className="flex items-center gap-2">
                  <Search className="h-3.5 w-3.5" />
                  Search cases, alerts, detections, agents...
                </span>
                <span className="flex items-center gap-1 rounded border border-border bg-secondary px-1.5 py-0.5 font-mono text-[10px]">
                  <Command className="h-3 w-3" /> K
                </span>
              </button>
            </div>

            <div className="flex items-center gap-2">
              <button className="grid h-8 w-8 place-items-center rounded-md border border-border bg-background/80 text-muted-foreground hover:text-foreground">
                <Bell className="h-4 w-4" />
              </button>
              <div className="hidden items-center gap-2 rounded-md border border-border bg-background/80 px-2 py-1.5 md:flex">
                <RadioTower className="h-3.5 w-3.5 text-signal-good" />
                <span className="font-mono text-[11px] text-muted-foreground">acme-prod</span>
              </div>
            </div>
          </header>

          <main className="min-h-0 flex-1 overflow-auto">{children}</main>
        </div>
      </div>
    </div>
  );
}
