"use client";

import {
  Activity,
  BarChart3,
  Bot,
  Crosshair,
  FileCode2,
  FlaskConical,
  Gauge,
  HelpCircle,
  Settings,
  ShieldCheck,
  Siren,
  Users,
} from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarSeparator,
} from "@/components/ui/sidebar";
import { NavSecondary } from "@/components/nav-secondary";
import { NavUser } from "@/components/nav-user";

const navMain = [
  { href: "/workbench/queue",      label: "Queue",      icon: Siren,      description: "Real-time alert queue" },
  { href: "/workbench/cases",      label: "Cases",      icon: ShieldCheck, description: "Investigation cases" },
  { href: "/workbench/hunt",       label: "Hunt",       icon: Crosshair,  description: "Threat hunting" },
  { href: "/workbench/detections", label: "Detections", icon: FileCode2,  description: "Detection rules" },
  { href: "/workbench/simulate",   label: "Simulate",   icon: FlaskConical, description: "ML triage sandbox" },
  { href: "/workbench/load",       label: "Load Lab",   icon: Gauge,      description: "Stress testing" },
];

const navSecondary = [
  { href: "/workbench/coverage",   label: "Coverage",   icon: BarChart3 },
  { href: "/workbench/agents",     label: "Agents",     icon: Bot },
  { href: "/workbench/settings",   label: "Settings",   icon: Settings },
  { href: "/workbench/admin",      label: "Admin",      icon: Users },
];

const user = {
  name: "Analyst",
  email: "analyst@attest.dev",
  avatar: "",
};

export function AttestSidebar() {
  const pathname = usePathname();

  const isActive = (href: string) =>
    href === "/workbench/cases"
      ? pathname.startsWith("/workbench/cases")
      : pathname === href || pathname.startsWith(`${href}/`);

  return (
    <Sidebar collapsible="offcanvas" variant="inset">
      {/* Header / Logo */}
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" asChild>
              <Link href="/workbench">
                <div className="grid h-8 w-8 shrink-0 place-items-center rounded-md border border-primary/30 bg-primary/15 text-primary">
                  <Activity className="h-4 w-4" />
                </div>
                <div className="flex flex-col leading-none">
                  <span className="font-semibold tracking-tight">Attest</span>
                  <span className="font-mono text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    Analyst Workbench
                  </span>
                </div>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        {/* Main nav */}
        <SidebarGroup>
          <SidebarGroupLabel>Operations</SidebarGroupLabel>
          <SidebarMenu>
            {navMain.map((item) => (
              <SidebarMenuItem key={item.href}>
                <SidebarMenuButton
                  asChild
                  tooltip={item.label}
                  isActive={isActive(item.href)}
                >
                  <Link href={item.href}>
                    <item.icon />
                    <span>{item.label}</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            ))}
          </SidebarMenu>
        </SidebarGroup>

        <SidebarSeparator />

        {/* Secondary nav */}
        <SidebarGroup>
          <SidebarGroupLabel>Platform</SidebarGroupLabel>
          <SidebarMenu>
            {navSecondary.map((item) => (
              <SidebarMenuItem key={item.href}>
                <SidebarMenuButton
                  asChild
                  tooltip={item.label}
                  isActive={isActive(item.href)}
                >
                  <Link href={item.href}>
                    <item.icon />
                    <span>{item.label}</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            ))}
          </SidebarMenu>
        </SidebarGroup>

        {/* Help — push to bottom */}
        <NavSecondary
          className="mt-auto"
          items={[
            { title: "Help & Docs", url: "/workbench/settings", icon: HelpCircle },
          ]}
        />
      </SidebarContent>

      {/* Footer — user profile */}
      <SidebarFooter>
        <NavUser user={user} />
      </SidebarFooter>
    </Sidebar>
  );
}
