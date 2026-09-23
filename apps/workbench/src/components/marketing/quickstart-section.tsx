"use client";

import { motion } from "framer-motion";
import { Check, Copy } from "lucide-react";
import { useState } from "react";
import { analytics } from "@/lib/analytics";

type Step = {
  id: string;
  label: string;
  cmd: string;
  out: { text: string; tone: "pass" | "fail" | "muted" }[];
};

const LOG = "examples/verify/attestations.ndjson";
const KEY = "--key $(cat examples/verify/verifying-key.txt)";

const steps: Step[] = [
  {
    id: "clone",
    label: "Clone",
    cmd: "git clone https://github.com/mhomaid/attest && cd attest",
    out: [],
  },
  {
    id: "verify",
    label: "Verify the sample log",
    cmd: `cargo run -q -p attest-cli -- verify ${LOG} ${KEY}`,
    out: [
      { text: "PASS  5a3e…0001  pass (classifier)", tone: "pass" },
      { text: "PASS  5a3e…0002  pass (classifier)", tone: "pass" },
      { text: "PASS  5a3e…0003  pass (classifier)", tone: "pass" },
      { text: "verified 3/3", tone: "muted" },
    ],
  },
  {
    id: "tamper",
    label: "Delete one row, verify again",
    cmd: `sed -i.bak '2d' ${LOG} && cargo run -q -p attest-cli -- verify ${LOG} ${KEY}`,
    out: [
      { text: "PASS  5a3e…0001  pass (classifier)", tone: "pass" },
      { text: "FAIL  5a3e…0003  chain break: prev_hash=1beb… expected=59f1…", tone: "fail" },
      { text: "verified 1/2", tone: "muted" },
    ],
  },
  {
    id: "pin",
    label: "Pin the wrong model",
    cmd: `git checkout ${LOG} && cargo run -q -p attest-cli -- verify ${LOG} ${KEY} --pin-model 0000`,
    out: [
      { text: "FAIL  5a3e…0001  model_artifact_hash swap: recorded=8dee… pinned=0000", tone: "fail" },
      { text: "verified 0/3", tone: "muted" },
    ],
  },
];

export function QuickstartSection() {
  return (
    <section id="quickstart" className="scroll-mt-16 border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.45 }}
        >
          <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
            Run it locally
          </p>
          <h2 className="mt-3 max-w-3xl text-2xl font-semibold tracking-tight sm:text-3xl">
            Break the log yourself. It only needs Rust.
          </h2>

          <ol className="mt-10 space-y-4">
            {steps.map((s, i) => (
              <li key={s.id} className="grid gap-3 sm:grid-cols-[12rem_minmax(0,1fr)]">
                <p className="pt-2.5 text-sm font-semibold">
                  <span className="mr-2 font-mono text-xs text-muted-foreground">{i + 1}</span>
                  {s.label}
                </p>
                <div className="overflow-hidden rounded-xl border border-border/70 bg-black/40">
                  <CommandLine step={s} />
                  {s.out.length > 0 ? (
                    <pre className="overflow-x-auto border-t border-border/50 px-4 py-3 font-mono text-[11px] leading-5">
                      {s.out.map((o) => (
                        <span
                          key={o.text}
                          className={
                            o.tone === "pass"
                              ? "block text-signal-good"
                              : o.tone === "fail"
                                ? "block text-red-400"
                                : "block text-muted-foreground"
                          }
                        >
                          {o.text}
                        </span>
                      ))}
                    </pre>
                  ) : null}
                </div>
              </li>
            ))}
          </ol>

          <p className="mt-6 text-sm text-muted-foreground">
            Want the whole stack: Kafka, RisingWave, triage and this workbench?{" "}
            <a
              href="https://github.com/mhomaid/attest#quick-start"
              target="_blank"
              rel="noreferrer"
              onClick={() => analytics.marketing_cta_clicked("quickstart_full_stack")}
              className="text-foreground underline underline-offset-4"
            >
              make dev-up-all
            </a>{" "}
            runs it on Docker.
          </p>
        </motion.div>
      </div>
    </section>
  );
}

function CommandLine({ step }: { step: Step }) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    await navigator.clipboard.writeText(step.cmd);
    analytics.marketing_quickstart_copied(step.id);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  }

  return (
    <div className="flex items-start gap-3 px-4 py-2.5">
      <code className="min-w-0 flex-1 whitespace-pre-wrap break-all font-mono text-[12px] leading-6 text-foreground">
        <span className="select-none text-muted-foreground">$ </span>
        {step.cmd}
      </code>
      <button
        type="button"
        onClick={copy}
        aria-label={`Copy: ${step.label}`}
        className="mt-0.5 shrink-0 rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
      >
        {copied ? <Check className="h-3.5 w-3.5 text-signal-good" /> : <Copy className="h-3.5 w-3.5" />}
      </button>
    </div>
  );
}
