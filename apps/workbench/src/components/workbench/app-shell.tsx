"use client";

import { Bell, Command, RadioTower, Search } from "lucide-react";
import Link from "next/link";
import type { CSSProperties, ReactNode } from "react";
import { AttestSidebar } from "@/components/workbench/attest-sidebar";
import { CommandPalette } from "@/components/workbench/command-palette";
import { SignOutButton } from "@/components/workbench/sign-out-button";
import { StatusBadge } from "@/components/workbench/status-badge";
import { Separator } from "@/components/ui/separator";
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { useCommandPalette } from "@/lib/stores/command-palette-store";

export function AppShell({ children }: { children: ReactNode }) {
  const { setOpen } = useCommandPalette();
  return (
    <SidebarProvider
      style={
        {
          /* Literal sizes: `calc(var(--spacing) * …)` needs a `--spacing` theme token some setups lack. */
          "--sidebar-width": "18rem",
          "--header-height": "3rem",
        } as CSSProperties
      }
    >
      <AttestSidebar />

      <SidebarInset className="min-w-0">
        {/* Portals to document; keep layout siblings = peer `Sidebar` + `SidebarInset` */}
        <CommandPalette />
        {/* ── Top header ── */}
        <header className="flex h-[var(--header-height,3rem)] shrink-0 items-center justify-between gap-2 border-b border-border/80 bg-card/60 px-3 backdrop-blur transition-[width,height] ease-linear">
          <div className="flex items-center gap-2">
            <SidebarTrigger className="-ml-1 text-muted-foreground hover:text-foreground" />
            <Separator orientation="vertical" className="mx-1 data-[orientation=vertical]:h-4" />
            <StatusBadge tone="good">Streaming live</StatusBadge>
            <StatusBadge tone="info">HELIQL + Arroyo</StatusBadge>
          </div>

          {/* CMD+K search bar */}
          <div className="flex min-w-0 flex-1 justify-center px-4">
            <button
              onClick={() => setOpen(true)}
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

          {/* Right side */}
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
              <span className="font-mono text-[11px] text-muted-foreground">Attest</span>
            </div>
            <SignOutButton />
          </div>
        </header>

        <div className="flex flex-1 flex-col">
          <div className="@container/main flex flex-1 flex-col gap-2">
            {children}
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
