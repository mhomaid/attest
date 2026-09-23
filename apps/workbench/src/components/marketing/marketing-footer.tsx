import { Shield } from "lucide-react";
import Link from "next/link";

const product = [
  { href: "/waitlist", label: "Request access" },
  { href: "/#try", label: "Try" },
  { href: "/#verify", label: "Verify" },
  { href: "/#pipeline", label: "Pipeline" },
  { href: "/#guards", label: "Guards" },
  { href: "/#detections", label: "HELIQL" },
  { href: "/#stack", label: "Planes" },
  { href: "/#deploy", label: "Deploy" },
  { href: "/#quickstart", label: "Quickstart" },
] as const;

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
            Open-source verifiable SOC. Every AI verdict is a signed object you can check.
          </p>
        </div>
        <div className="grid grid-cols-2 gap-10">
          <div>
            <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
              Product
            </p>
            <ul className="mt-3 space-y-2 text-sm">
              {product.map((p) => (
                <li key={p.href}>
                  <Link href={p.href} className="text-muted-foreground hover:text-foreground">
                    {p.label}
                  </Link>
                </li>
              ))}
            </ul>
          </div>
          <div>
            <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
              <Link href="/stack" className="hover:text-foreground">
                Stack
              </Link>
            </p>
            <ul className="mt-3 space-y-2 text-sm text-muted-foreground">
              <li>Rust · axum 0.8 · tract-onnx</li>
              <li>Kafka / Redpanda · RisingWave · Arroyo</li>
              <li>OCSF 1.3 · Iceberg · ClickHouse</li>
              <li>Next.js 16 · Bun · Postgres 18</li>
              <li>
                <Link href="/stack" className="text-foreground hover:underline">
                  All components →
                </Link>
              </li>
            </ul>
          </div>
        </div>
      </div>
      <div className="mx-auto mt-10 max-w-6xl border-t border-border/50 px-4 pt-6 sm:px-6">
        <p className="text-center text-xs text-muted-foreground">
          © {new Date().getFullYear()} Attest · Apache-2.0
        </p>
      </div>
    </footer>
  );
}
