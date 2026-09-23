"use client";

import { motion } from "framer-motion";
import { ArrowDown, Loader2, Play } from "lucide-react";
import { useEffect, useState } from "react";
import { analytics } from "@/lib/analytics";
import { cn } from "@/lib/utils";

type DemoResult = {
  ok: boolean;
  error?: string;
  action_id: string;
  verdict: string;
  execution_path: string;
  tenant_id: string;
  envelope?: {
    agent_action_id: string;
    tenant_id: string;
    verdict: string;
    prev_hash: string;
    signature: string;
    signed_at: string;
  };
  verify?: {
    ok: boolean;
    passed: number;
    total: number;
    verifying_key: string;
    durable: boolean;
    results: { action_id: string; ok: boolean; detail: string }[];
  };
  collector: { ok: boolean; error?: string };
  orchestrator: { ok: boolean; error?: string };
};

export function TryDemoSection() {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<DemoResult | null>(null);
  const [live, setLive] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetch("/api/demo/status", { cache: "no-store" })
      .then((r) => r.json() as Promise<{ live: boolean }>)
      .then((s) => {
        if (!cancelled) setLive(s.live);
      })
      .catch(() => {
        if (!cancelled) setLive(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function run() {
    setBusy(true);
    setResult(null);
    analytics.marketing_cta_clicked("try_demo_run");
    try {
      const r = await fetch("/api/demo/run", { method: "POST" });
      const data = (await r.json()) as DemoResult;
      setResult(data);
      analytics.marketing_demo_step("try", data.ok ? "pass" : "fail");
    } catch (e) {
      setResult({
        ok: false,
        error: String(e),
        action_id: "",
        verdict: "unknown",
        execution_path: "classifier",
        tenant_id: "demo",
        collector: { ok: false, error: String(e) },
        orchestrator: { ok: false },
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <section id="try" className="scroll-mt-16 border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.45 }}
        >
          <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
            Public demo
          </p>
          <h2 className="mt-3 max-w-3xl text-2xl font-semibold tracking-tight sm:text-3xl">
            CloudTrail in. Signed verdict out. Verify it here.
          </h2>
          <p className="mt-3 max-w-2xl text-sm leading-6 text-muted-foreground">
            Runs the hosted demo tenant: ingest a geo-anomaly ConsoleLogin, triage it, then walk the
            hash chain with the same verifier the CLI uses. If the collector or orchestrator is
            down, this page says so instead of faking a pass.
          </p>

          {live === false ? (
            <div className="mt-8 max-w-2xl rounded-xl border border-border/70 bg-card/40 p-5">
              <p className="text-sm font-medium">
                The hosted demo is paused to keep hosting costs down.
              </p>
              <p className="mt-2 text-sm leading-6 text-muted-foreground">
                Attest is an early open-source prototype, not a production service. The same
                check runs on your machine in about two minutes with only Rust installed: verify
                a signed log, delete one row, and watch{" "}
                <code className="font-mono text-foreground">attest verify</code> fail.
              </p>
              <a
                href="#quickstart"
                onClick={() => analytics.marketing_cta_clicked("try_demo_paused_quickstart")}
                className="mt-4 inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2.5 text-sm font-medium text-primary-foreground"
              >
                Check it yourself
                <ArrowDown className="h-4 w-4" />
              </a>
            </div>
          ) : (
            <>
              <div className="mt-8 flex flex-wrap items-center gap-3">
                <button
                  type="button"
                  onClick={run}
                  disabled={busy || live === null}
                  className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2.5 text-sm font-medium text-primary-foreground disabled:opacity-60"
                >
                  {busy || live === null ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Play className="h-4 w-4" />
                  )}
                  {busy ? "Signing…" : live === null ? "Checking demo…" : "Run the demo"}
                </button>
                <p className="font-mono text-[11px] text-muted-foreground">
                  tenant=demo · scenario=geo_anomaly
                </p>
              </div>

              {result ? <DemoResultCard result={result} /> : null}
            </>
          )}
        </motion.div>
      </div>
    </section>
  );
}

function DemoResultCard({ result }: { result: DemoResult }) {
  const env = result.envelope;
  const verify = result.verify;

  return (
    <div className="mt-8 grid gap-4 lg:grid-cols-2">
      <div className="rounded-xl border border-border/70 bg-black/40 p-4">
        <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
          Envelope
        </p>
        {env ? (
          <dl className="mt-3 space-y-1.5 font-mono text-[11px]">
            <Row k="tenant_id" v={env.tenant_id} />
            <Row k="verdict" v={env.verdict} />
            <Row k="action_id" v={short(env.agent_action_id)} />
            <Row k="prev_hash" v={short(env.prev_hash)} />
            <Row k="signature" v={short(env.signature)} />
          </dl>
        ) : (
          <p className="mt-3 text-sm text-muted-foreground">
            {result.orchestrator.error ?? "No envelope yet."}
          </p>
        )}
      </div>

      <div className="rounded-xl border border-border/70 bg-black/40 p-4">
        <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
          attest verify
        </p>
        {verify ? (
          <>
            <p
              className={cn(
                "mt-3 font-mono text-sm",
                verify.ok ? "text-signal-good" : "text-red-400",
              )}
            >
              {verify.ok ? "PASS" : "FAIL"} · verified {verify.passed}/{verify.total}
              {verify.durable ? " · object store" : " · local log"}
            </p>
            <ul className="mt-3 space-y-1 font-mono text-[11px] text-muted-foreground">
              {verify.results.slice(-3).map((row) => (
                <li key={row.action_id}>
                  <span className={row.ok ? "text-signal-good" : "text-red-400"}>
                    {row.ok ? "PASS" : "FAIL"}
                  </span>{" "}
                  {short(row.action_id)} · {row.detail}
                </li>
              ))}
            </ul>
          </>
        ) : (
          <FailureNotes result={result} />
        )}
      </div>
    </div>
  );
}

function FailureNotes({ result }: { result: DemoResult }) {
  return (
    <div className="mt-3 space-y-2 text-sm text-muted-foreground">
      <p>{result.error ?? "Demo services are not reachable from this host."}</p>
      {!result.collector.ok ? (
        <p>
          Collector: {result.collector.error ?? "down"}. Clone the repo and run{" "}
          <code className="text-foreground">make dev-up-all</code>, or verify the sample log in
          Quickstart.
        </p>
      ) : null}
    </div>
  );
}

function Row({ k, v }: { k: string; v: string }) {
  return (
    <div className="flex gap-3">
      <dt className="w-24 shrink-0 text-muted-foreground">{k}</dt>
      <dd className="truncate text-foreground">{v}</dd>
    </div>
  );
}

function short(value: string) {
  if (!value) return "—";
  if (value.length <= 16) return value;
  return `${value.slice(0, 8)}…${value.slice(-4)}`;
}
