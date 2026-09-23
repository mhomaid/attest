"use client";

import { motion } from "framer-motion";
import { Menu, Shield } from "lucide-react";
import Link from "next/link";
import { useState } from "react";
import { analytics } from "@/lib/analytics";
import { cn } from "@/lib/utils";

const links = [
  { href: "/#try", label: "Try" },
  { href: "/#verify", label: "Verify" },
  { href: "/#pipeline", label: "Pipeline" },
  { href: "/#guards", label: "Guards" },
  { href: "/#detections", label: "Detections" },
  { href: "/stack", label: "Stack" },
  { href: "https://github.com/mhomaid/attest", label: "GitHub" },
] as const;

export function MarketingNav() {
  const [open, setOpen] = useState(false);

  return (
    <motion.header
      initial={{ y: -12, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}
      className="fixed top-0 z-50 w-full border-b border-border/50 bg-background/70 backdrop-blur-md"
    >
      <div className="mx-auto flex h-14 max-w-6xl items-center justify-between gap-4 px-4 sm:px-6">
        <Link href="/" className="flex items-center gap-2 font-semibold tracking-tight">
          <span className="grid h-8 w-8 place-items-center rounded-md border border-primary/35 bg-primary/10 text-primary">
            <Shield className="h-4 w-4" />
          </span>
          <span>Attest</span>
        </Link>

        <nav className="hidden items-center gap-1 md:flex">
          {links.map((item) => (
            <Link
              key={item.href}
              href={item.href}
              className="rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
            >
              {item.label}
            </Link>
          ))}
        </nav>

        <div className="flex items-center gap-2">
          <Link
            href="/waitlist"
            className="hidden rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground sm:inline-flex"
          >
            Request access
          </Link>
          <Link
            href="/#quickstart"
            onClick={() => analytics.marketing_cta_clicked("nav_run_locally")}
            className="inline-flex rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground"
          >
            Run it locally
          </Link>
          <button
            type="button"
            className="rounded-md p-2 md:hidden"
            aria-label="Open menu"
            onClick={() => setOpen((o) => !o)}
          >
            <Menu className="h-5 w-5" />
          </button>
        </div>
      </div>

      <div
        className={cn(
          "border-t border-border/50 bg-background/95 px-4 py-3 md:hidden",
          !open && "hidden",
        )}
      >
        <div className="flex flex-col gap-1">
          {links.map((item) => (
            <Link
              key={item.href}
              href={item.href}
              className="rounded-md px-2 py-2 text-sm"
              onClick={() => setOpen(false)}
            >
              {item.label}
            </Link>
          ))}
          <Link
            href="/waitlist"
            className="rounded-md px-2 py-2 text-sm text-muted-foreground"
            onClick={() => setOpen(false)}
          >
            Request access
          </Link>
        </div>
      </div>
    </motion.header>
  );
}
