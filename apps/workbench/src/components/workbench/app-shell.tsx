"use client";

import {
  Activity,
  Bell,
  Bot,
  Command,
  Crosshair,
  FileCode2,
  FlaskConical,
  Gauge,
  RadioTower,
  Search,
  Settings,
  ShieldCheck,
  Siren,
  Users,
} from "lucide-react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, type ReactNode } from "react";
import { StatusBadge } from "@/components/workbench/status-badge";
import { cn } from "@/lib/utils";

const navItems = [
  { href: "/workbench/queue",      label: "Queue",      icon: Siren },
  { href: "/workbench/cases",      label: "Cases",      icon: ShieldCheck },
  { href: "/workbench/hunt",       label: "Hunt",       icon: Crosshair },
  { href: "/workbench/detections", label: "Detections", icon: FileCode2 },
  { href: "/workbench/simulate",   label: "Simulate",   icon: FlaskConical },
  { href: "/workbench/coverage",   label: "Coverage",   icon: Gauge },
  { href: "/workbench/agents",     label: "Agents",     icon: Bot },
  { href: "/workbench/settings",   label: "Settings",   icon: Settings },
  { href: "/workbench/admin",      label: "Admin",      icon: Users },
];

export function AppShell({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const router   = useRouter();

  // ⌘K / Ctrl+K global shortcut
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        router.push("/workbench/queue");
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [router]);

  const isActive = (href: string) =>
    href === "/workbench/cases"
      ? pathname.startsWith("/workbench/cases")
      : pathname === href || pathname.startsWith(`${href}/`);

  return (
    <div className="min-h-screen overflow-hidden bg-background text-foreground">
      <div className="flex min-h-screen">
        {/* ── Sidebar ── */}
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
                  "group flex items-center gap-2 rounded-md px-2 py-2 text-sm text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground",
                  isActive(item.href) && "bg-secondary text-foreground font-medium",
                )}
              >
                <item.icon className="h-4 w-4 shrink-0" />
                {item.label}
              </Link>
            ))}
          </nav>
        </aside>

        {/* ── Main area ── */}
        <div className="flex min-w-0 flex-1 flex-col">
          <header className="flex h-14 shrink-0 items-center justify-between border-b border-border/80 bg-card/60 px-3 backdrop-blur">
            <div className="flex items-center gap-2">
              <StatusBadge tone="good">Streaming live</StatusBadge>
              <StatusBadge tone="info">HELIQL + Arroyo</StatusBadge>
            </div>

            <div className="flex min-w-0 flex-1 justify-center px-4">
              <button
                onClick={() => router.push("/workbench/queue")}
                className="flex h-8 w-full max-w-xl items-center justify-between rounded-md border border-border bg-background/80 px-2 text-left text-xs text-muted-foreground shadow-sm transition-colors hover:border-border/80 hover:bg-secondary/50"
              >
                <span className="flex items-center gap-2">
                  <Search className="h-3.5 w-3.5" />
                  Search cases, alerts, detections...
                </span>
                <span className="flex items-center gap-1 rounded border border-border bg-secondary px-1.5 py-0.5 font-mono text-[10px]">
                  <Command className="h-3 w-3" /> K
                </span>
              </button>
            </div>

            <div className="flex items-center gap-2">
              <Link
                href="/workbench/queue"
                className="grid h-8 w-8 place-items-center rounded-md border border-border bg-background/80 text-muted-foreground transition-colors hover:text-foreground"
                title="Alert Queue"
              >
                <Bell className="h-4 w-4" />
              </Link>
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
