"use client";

import {
  Bot,
  BarChart3,
  Crosshair,
  FileCode2,
  FlaskConical,
  Gauge,
  Settings,
  ShieldCheck,
  Siren,
  Users,
  ExternalLink,
} from "lucide-react";
import { useRouter } from "next/navigation";
import { useEffect } from "react";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@/components/ui/command";

const pages = [
  { href: "/workbench/queue",      label: "Queue",      description: "Real-time alert queue",            icon: Siren,      shortcut: "Q" },
  { href: "/workbench/cases",      label: "Cases",      description: "Open investigation cases",         icon: ShieldCheck, shortcut: null },
  { href: "/workbench/hunt",       label: "Hunt",       description: "Ad-hoc threat hunting queries",    icon: Crosshair,  shortcut: null },
  { href: "/workbench/detections", label: "Detections", description: "Detection rules and coverage",     icon: FileCode2,  shortcut: "D" },
  { href: "/workbench/simulate",   label: "Simulate",   description: "ML triage sandbox",                icon: FlaskConical, shortcut: null },
  { href: "/workbench/load",       label: "Load Lab",   description: "Stress test the pipeline",         icon: Gauge,      shortcut: null },
  { href: "/workbench/coverage",   label: "Coverage",   description: "MITRE ATT&CK coverage map",        icon: BarChart3,  shortcut: null },
  { href: "/workbench/agents",     label: "Agents",     description: "AI agent status and attestations", icon: Bot,        shortcut: null },
  { href: "/workbench/settings",   label: "Settings",   description: "Platform configuration",           icon: Settings,   shortcut: null },
  { href: "/workbench/admin",      label: "Admin",      description: "Service health and OpenAPI docs",  icon: Users,      shortcut: null },
];

const externalLinks = [
  { href: "http://localhost:4000/docs",  label: "Collector API Docs",      shortcut: null },
  { href: "http://localhost:8080/docs",  label: "Control Plane API Docs",  shortcut: null },
  { href: "http://localhost:4300/docs",  label: "Orchestrator API Docs",   shortcut: null },
  { href: "http://localhost:5115",       label: "Arroyo UI",               shortcut: null },
  { href: "http://localhost:8081",       label: "Redpanda Console",        shortcut: null },
];

import { useCommandPalette } from "@/lib/stores/command-palette-store";

export function CommandPalette() {
  const { open, setOpen } = useCommandPalette();
  const router = useRouter();

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setOpen(true);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [setOpen]);

  function navigate(href: string) {
    setOpen(false);
    if (href.startsWith("http")) {
      window.open(href, "_blank", "noopener,noreferrer");
    } else {
      router.push(href);
    }
  }

  return (
    <CommandDialog
      open={open}
      onOpenChange={setOpen}
      title="Search"
      description="Search workbench pages and open external tools. Arrow keys to move, Enter to open, Escape to close."
    >
      <CommandInput placeholder="Search pages, alerts, detections..." />
      <CommandList>
        <CommandEmpty>No results found.</CommandEmpty>

        <CommandGroup heading="Pages">
          {pages.map((page) => (
            <CommandItem
              key={page.href}
              value={`${page.label} ${page.description}`}
              onSelect={() => navigate(page.href)}
              className="gap-3"
            >
              <page.icon className="h-4 w-4 text-muted-foreground" />
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium">{page.label}</div>
                <div className="text-xs text-muted-foreground">{page.description}</div>
              </div>
              {page.shortcut && <CommandShortcut>⌘{page.shortcut}</CommandShortcut>}
            </CommandItem>
          ))}
        </CommandGroup>

        <CommandSeparator />

        <CommandGroup heading="External Tools">
          {externalLinks.map((link) => (
            <CommandItem
              key={link.href}
              value={link.label}
              onSelect={() => navigate(link.href)}
              className="gap-3"
            >
              <ExternalLink className="h-4 w-4 text-muted-foreground" />
              <span className="text-sm">{link.label}</span>
            </CommandItem>
          ))}
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  );
}
