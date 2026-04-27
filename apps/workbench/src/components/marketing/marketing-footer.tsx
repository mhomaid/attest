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
            Attest: The Verifiable Agentic SIEM for the post-human threat landscape.
          </p>
        </div>
        <div className="grid grid-cols-2 gap-10 sm:grid-cols-3">
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
              Blueprint
            </p>
            <ul className="mt-3 space-y-2 text-sm text-muted-foreground">
              <li>PRD, architecture, build order, and workbench spec live in the repo</li>
            </ul>
          </div>
          <div>
            <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
              UI
            </p>
            <p className="mt-3 text-sm text-muted-foreground">
              Marketing motion and component patterns inspired by{" "}
              <a
                href="https://ui.aceternity.com/"
                className="text-foreground underline-offset-2 hover:underline"
                target="_blank"
                rel="noreferrer"
              >
                Aceternity UI
              </a>{" "}
              (Framer Motion + Tailwind). The SOC console under{" "}
              <code className="rounded bg-secondary px-1 font-mono text-xs">/workbench</code>{" "}
              stays information-dense per product spec.
            </p>
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
