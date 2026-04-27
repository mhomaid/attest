import { Shield } from "lucide-react";
import Link from "next/link";

export function MarketingFooter() {
  return (
    <footer className="border-t border-border/60 bg-card/30 py-12">
      <div className="mx-auto flex max-w-6xl flex-col gap-8 px-4 sm:flex-row sm:items-start sm:justify-between sm:px-6">
        <div>
          <div className="flex items-center gap-2 font-semibold">
            <span className="grid h-8 w-8 place-items-center rounded-md border border-border bg-secondary">
              <Shield className="h-4 w-4" />
            </span>
            Attest
          </div>
          <p className="mt-2 max-w-xs text-sm text-muted-foreground">
            The Verifiable Agentic SIEM for the post-human threat landscape.
          </p>
        </div>
        <div className="grid grid-cols-2 gap-10">
          <div>
            <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
              Product
            </p>
            <ul className="mt-3 space-y-2 text-sm">
              <li>
                <a href="#pillars" className="text-muted-foreground hover:text-foreground">
                  Platform
                </a>
              </li>
              <li>
                <a href="#fabric" className="text-muted-foreground hover:text-foreground">
                  Detection
                </a>
              </li>
              <li>
                <Link
                  href="/workbench/queue"
                  className="text-muted-foreground hover:text-foreground"
                >
                  Workbench
                </Link>
              </li>
            </ul>
          </div>
          <div>
            <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
              Stack
            </p>
            <ul className="mt-3 space-y-2 text-sm text-muted-foreground">
              <li>Rust 1.95 · axum 0.8</li>
              <li>Redpanda · RisingWave</li>
              <li>OCSF 1.3 · ClickHouse</li>
              <li>Next.js 15 · Bun · shadcn</li>
            </ul>
          </div>
        </div>
      </div>
      <div className="mx-auto mt-10 max-w-6xl border-t border-border/50 px-4 pt-6 sm:px-6">
        <p className="text-center text-xs text-muted-foreground">
          © {new Date().getFullYear()} Attest. All rights reserved.
        </p>
      </div>
    </footer>
  );
}
