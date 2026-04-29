"use client";

import {
  CheckCircle2,
  Clock3,
  Database,
  FileSignature,
  ShieldAlert,
  ThumbsDown,
  ThumbsUp,
  X,
} from "lucide-react";
import { useState } from "react";
import { ClassifierEvidencePanel } from "@/components/workbench/classifier-evidence-panel";
import { StatusBadge } from "@/components/workbench/status-badge";
import type { CaseRecord, Severity } from "@/lib/mock-data";
import { cn } from "@/lib/utils";

const severityTone: Record<Severity, "critical" | "high" | "medium" | "low"> = {
  critical: "critical",
  high:     "high",
  medium:   "medium",
  low:      "low",
};

// ── Verdict override dialog ───────────────────────────────────────────────────

type OverrideLabel = "true_positive" | "false_positive" | "benign" | "needs_investigation";

function VerdictDialog({
  onClose,
  onSubmit,
}: {
  onClose: () => void;
  onSubmit: (label: OverrideLabel, reason: string) => void;
}) {
  const [label, setLabel]   = useState<OverrideLabel>("true_positive");
  const [reason, setReason] = useState("");
  const [busy, setBusy]     = useState(false);

  const labels: { value: OverrideLabel; display: string }[] = [
    { value: "true_positive",      display: "True Positive" },
    { value: "false_positive",     display: "False Positive" },
    { value: "benign",             display: "Benign" },
    { value: "needs_investigation",display: "Needs Investigation" },
  ];

  const handleSubmit = async () => {
    if (!reason.trim()) return;
    setBusy(true);
    // Simulate a brief network round-trip for now; Phase 4b wires real API.
    await new Promise((r) => setTimeout(r, 300));
    onSubmit(label, reason);
    setBusy(false);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm">
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-5 shadow-xl">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-sm font-semibold">Override Verdict</h2>
          <button
            onClick={onClose}
            className="grid h-6 w-6 place-items-center rounded text-muted-foreground hover:text-foreground"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="space-y-3">
          <div>
            <label className="mb-1 block text-xs font-medium text-muted-foreground">
              Override label
            </label>
            <div className="grid grid-cols-2 gap-2">
              {labels.map((l) => (
                <button
                  key={l.value}
                  onClick={() => setLabel(l.value)}
                  className={cn(
                    "rounded-md border px-3 py-2 text-left text-xs transition-colors",
                    label === l.value
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-border bg-background/60 text-muted-foreground hover:border-border/80 hover:bg-secondary/50",
                  )}
                >
                  {l.display}
                </button>
              ))}
            </div>
          </div>

          <div>
            <label className="mb-1 block text-xs font-medium text-muted-foreground">
              Reason <span className="text-destructive">*</span>
            </label>
            <textarea
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="Confirmed benign by user, legitimate travel pattern..."
              rows={3}
              className="w-full resize-none rounded-md border border-border bg-background/60 px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30"
            />
          </div>

          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="flex-1 rounded-md border border-border px-3 py-2 text-xs text-muted-foreground transition-colors hover:bg-secondary/50"
            >
              Cancel
            </button>
            <button
              onClick={handleSubmit}
              disabled={!reason.trim() || busy}
              className="flex-1 rounded-md bg-primary px-3 py-2 text-xs font-medium text-primary-foreground transition-opacity disabled:opacity-40"
            >
              {busy ? "Submitting…" : "Submit Override"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export function CaseWorkbench({ caseRecord }: { caseRecord: CaseRecord }) {
  const [overrideOpen,    setOverrideOpen]    = useState(false);
  const [overrideApplied, setOverrideApplied] = useState<{
    label: OverrideLabel;
    reason: string;
  } | null>(null);
  const [approved, setApproved] = useState(false);

  const handleApprove = () => setApproved(true);

  const handleOverrideSubmit = (label: OverrideLabel, reason: string) => {
    setOverrideApplied({ label, reason });
    setOverrideOpen(false);
  };

  return (
    <>
      {overrideOpen && (
        <VerdictDialog
          onClose={() => setOverrideOpen(false)}
          onSubmit={handleOverrideSubmit}
        />
      )}

      <div className="grid min-h-full gap-3 p-3 xl:grid-cols-[minmax(0,1fr)_24rem]">
        {/* ── Left column ── */}
        <div className="min-w-0 space-y-3">
          {/* Case header */}
          <section className="rounded-lg border border-border bg-card/80 p-3">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div>
                <div className="mb-2 flex flex-wrap items-center gap-2">
                  <StatusBadge tone={severityTone[caseRecord.severity]}>
                    {caseRecord.severity}
                  </StatusBadge>
                  <StatusBadge tone="medium">
                    {overrideApplied
                      ? `Override: ${overrideApplied.label.replace(/_/g, " ")}`
                      : approved
                      ? "Approved"
                      : caseRecord.state.replace("-", " ")}
                  </StatusBadge>
                  <StatusBadge tone="info">{caseRecord.executionPath}</StatusBadge>
                </div>
                <h1 className="text-xl font-semibold tracking-tight">{caseRecord.title}</h1>
                <p className="mt-1 text-sm text-muted-foreground">
                  {caseRecord.entity} · {caseRecord.source}
                </p>
              </div>

              <div className="rounded-md border border-border bg-background/70 px-3 py-2 text-right">
                <div className="metric-tabular font-mono text-2xl font-semibold">
                  {Math.round(caseRecord.confidence * 100)}%
                </div>
                <div className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                  calibrated confidence
                </div>
              </div>
            </div>

            {/* Analyst action buttons */}
            {!overrideApplied && !approved && (
              <div className="mt-3 flex flex-wrap gap-2 border-t border-border pt-3">
                <button
                  onClick={handleApprove}
                  className="inline-flex items-center gap-1.5 rounded-md bg-signal-good/10 px-3 py-1.5 text-xs font-medium text-signal-good ring-1 ring-signal-good/30 transition-colors hover:bg-signal-good/20"
                >
                  <ThumbsUp className="h-3.5 w-3.5" />
                  Approve verdict
                </button>
                <button
                  onClick={() => setOverrideOpen(true)}
                  className="inline-flex items-center gap-1.5 rounded-md bg-severity-medium/10 px-3 py-1.5 text-xs font-medium text-severity-medium ring-1 ring-severity-medium/30 transition-colors hover:bg-severity-medium/20"
                >
                  <ThumbsDown className="h-3.5 w-3.5" />
                  Override verdict
                </button>
              </div>
            )}

            {/* Post-action confirmation */}
            {(overrideApplied || approved) && (
              <div className="mt-3 flex items-start gap-2 rounded-md border border-signal-good/30 bg-signal-good/10 px-3 py-2">
                <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-signal-good" />
                <div className="text-xs">
                  <span className="font-medium text-signal-good">
                    {overrideApplied ? "Override recorded" : "Verdict approved"}
                  </span>
                  {overrideApplied && (
                    <p className="mt-0.5 text-muted-foreground">
                      {overrideApplied.label.replace(/_/g, " ")} — {overrideApplied.reason}
                    </p>
                  )}
                  <p className="mt-0.5 text-muted-foreground">
                    A new signed attestation envelope has been recorded. Original verdict preserved.
                  </p>
                </div>
              </div>
            )}
          </section>

          {/* Reasoning trace */}
          <section className="rounded-lg border border-border bg-card/80">
            <header className="flex items-center justify-between border-b border-border px-3 py-2">
              <div className="flex items-center gap-2">
                <FileSignature className="h-4 w-4 text-muted-foreground" />
                <h2 className="text-sm font-semibold">Reasoning Trace</h2>
              </div>
              <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                signed envelope replay
              </span>
            </header>
            <div className="divide-y divide-border/80">
              {caseRecord.timeline.map((item) => (
                <div
                  key={`${item.time}-${item.actor}`}
                  className="grid gap-3 px-3 py-3 md:grid-cols-[5rem_8rem_minmax(0,1fr)]"
                >
                  <div className="flex items-center gap-1 font-mono text-[11px] text-muted-foreground">
                    <Clock3 className="h-3 w-3" />
                    {item.time}
                  </div>
                  <div className="text-xs font-medium">{item.actor}</div>
                  <div className="text-sm text-muted-foreground">{item.event}</div>
                </div>
              ))}
            </div>
          </section>
        </div>

        {/* ── Right sidebar ── */}
        <aside className="space-y-3">
          {/* Evidence */}
          <section className="rounded-lg border border-border bg-card/80">
            <header className="flex items-center gap-2 border-b border-border px-3 py-2">
              <Database className="h-4 w-4 text-muted-foreground" />
              <h2 className="text-sm font-semibold">Pinned Evidence</h2>
            </header>
            <div className="divide-y divide-border/80">
              {caseRecord.evidence.map((item) => (
                <div key={item.label} className="px-3 py-2">
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs font-medium">{item.label}</span>
                    <StatusBadge tone="muted">{item.type}</StatusBadge>
                  </div>
                  <p className="mt-1 text-xs leading-5 text-muted-foreground">{item.value}</p>
                </div>
              ))}
            </div>
          </section>

          {/* SHAP feature attribution */}
          <ClassifierEvidencePanel features={caseRecord.featureImpacts} />

          {/* Attestation status */}
          <section className="rounded-lg border border-signal-good/30 bg-signal-good/10 p-3">
            <div className="flex items-start gap-2">
              <CheckCircle2 className="mt-0.5 h-4 w-4 text-signal-good" />
              <div>
                <h2 className="text-sm font-semibold text-signal-good">Attestation Valid</h2>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">
                  Classifier envelope signature verified. Original verdict is immutable; analyst
                  overrides create a new signed envelope.
                </p>
              </div>
            </div>
          </section>

          {/* Edge state notice */}
          <section className="rounded-lg border border-severity-medium/30 bg-severity-medium/10 p-3">
            <div className="flex items-start gap-2">
              <ShieldAlert className="mt-0.5 h-4 w-4 text-severity-medium" />
              <div>
                <h2 className="text-sm font-semibold text-severity-medium">Calibration Note</h2>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">
                  Calibration model is more than 3 days old. Confidence scores may be slightly
                  inflated until the nightly recalibration run completes.
                </p>
              </div>
            </div>
          </section>
        </aside>
      </div>
    </>
  );
}
