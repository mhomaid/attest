"use client";

import {
  CheckCircle2,
  Database,
  FileSignature,
  LoaderCircle,
  ShieldAlert,
  ThumbsDown,
  ThumbsUp,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {
  InvestigatorTraceStepper,
  TriagePipelineStepper,
} from "@/components/workbench/investigator-trace-stepper";
import {
  ReasoningTraceViewer,
  type HttpTraceStep,
} from "@/components/workbench/reasoning-trace-viewer";
import { StatusBadge } from "@/components/workbench/status-badge";
import { analytics } from "@/lib/analytics";
import type { CaseRecord, Severity } from "@/lib/mock-data";
import { useCaseStore, selectDisposition } from "@/lib/stores/case-store";
import { cn } from "@/lib/utils";
import type { KafkaTraceStep } from "@/hooks/use-case-trace-ws";

const severityTone: Record<Severity, "critical" | "high" | "medium" | "low"> = {
  critical: "critical",
  high: "high",
  medium: "medium",
  low: "low",
};

type OverrideLabel = "true_positive" | "false_positive" | "benign" | "needs_investigation";

function httpTraceToKafkaStep(caseId: string, step: HttpTraceStep): KafkaTraceStep {
  return {
    case_id: caseId,
    tenant_id: "default",
    agent_action_id: step.agent_action_id,
    agent_id: step.agent_id,
    execution_path: step.execution_path,
    step_kind: step.kind,
    summary: step.verdict,
    ts: 0,
  };
}

function mergeTraceSteps(caseId: string, liveSteps: KafkaTraceStep[], httpSteps: HttpTraceStep[]) {
  const seen = new Set<string>();
  const merged: KafkaTraceStep[] = [];
  for (const step of [...liveSteps, ...httpSteps.map((s) => httpTraceToKafkaStep(caseId, s))]) {
    const key = `${step.agent_action_id}:${step.step_kind}:${step.ts}:${step.summary}`;
    if (seen.has(key)) continue;
    seen.add(key);
    merged.push(step);
  }
  return merged;
}

function VerdictDialog({
  onClose,
  onSubmit,
}: {
  onClose: () => void;
  onSubmit: (label: OverrideLabel, reason: string) => void;
}) {
  const [label, setLabel] = useState<OverrideLabel>("true_positive");
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);

  const labels: { value: OverrideLabel; display: string }[] = [
    { value: "true_positive", display: "True Positive" },
    { value: "false_positive", display: "False Positive" },
    { value: "benign", display: "Benign" },
    { value: "needs_investigation", display: "Needs Investigation" },
  ];

  const handleSubmit = async () => {
    if (!reason.trim()) return;
    setBusy(true);
    try {
      await Promise.resolve(onSubmit(label, reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm">
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-5 shadow-xl">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-sm font-semibold">Override Verdict</h2>
          <button
            type="button"
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
                  type="button"
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
              type="button"
              onClick={onClose}
              className="flex-1 rounded-md border border-border px-3 py-2 text-xs text-muted-foreground transition-colors hover:bg-secondary/50"
            >
              Cancel
            </button>
            <button
              type="button"
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

export function CaseWorkbench({
  caseRecord,
  orchestratorCaseId,
  liveSteps = [],
  triageLoading = false,
}: {
  caseRecord: CaseRecord;
  orchestratorCaseId?: string;
  liveSteps?: KafkaTraceStep[];
  /** True while the client-side triage fetch is in flight. */
  triageLoading?: boolean;
}) {
  const [overrideOpen, setOverrideOpen] = useState(false);
  const [approved, setApproved] = useState(false);
  // Local copy so the UI reflects the override even when the case is not in the store yet.
  const [localOverrideLabel, setLocalOverrideLabel] = useState<string | null>(null);
  const [httpSteps, setHttpSteps] = useState<HttpTraceStep[]>([]);

  const oid = orchestratorCaseId ?? "";
  const disposition = useCaseStore(selectDisposition(oid));
  const storeSetApproved = useCaseStore((s) => s.setApproved);
  const storeSetOverride = useCaseStore((s) => s.setOverride);

  const loadTrace = useCallback(() => {
    if (!oid) return;
    void fetch(`/api/cases/${encodeURIComponent(oid)}/trace`)
      .then((r) => r.json())
      .then((j) => setHttpSteps((j?.steps as HttpTraceStep[]) ?? []))
      .catch(() => setHttpSteps([]));
  }, [oid]);

  useEffect(() => {
    loadTrace();
  }, [loadTrace]);

  useEffect(() => {
    if (!triageLoading) loadTrace();
  }, [loadTrace, triageLoading]);

  const overrideApplied = localOverrideLabel !== null || disposition === "override";
  const approvedUi = approved || disposition === "approved";

  const handleApprove = () => {
    setApproved(true);
    if (oid) storeSetApproved(oid);
    analytics.verdict_approved();
    toast.success("Verdict approved");
  };

  const handleOverrideSubmit = (label: OverrideLabel, reason: string) => {
    if (!oid) return;
    setOverrideOpen(false);
    setLocalOverrideLabel(label);
    storeSetOverride(oid, label);
    analytics.verdict_overridden();
    toast.success("Override submitted");

    void fetch(`/api/cases/${encodeURIComponent(oid)}/override`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ label, reason }),
    })
      .then(async (res) => {
        if (!res.ok) {
          const j = await res.json().catch(() => ({}));
          toast.error(typeof j?.error === "string" ? j.error : "Override failed");
        } else {
          loadTrace();
        }
      })
      .catch(() => toast.error("Override failed"));
  };

  const storeOverrideLabel = useCaseStore((s) => s.cases[oid]?.overrideLabel ?? "");
  const overrideLabel = localOverrideLabel ?? storeOverrideLabel;
  const statusLabel = overrideApplied
    ? `Override: ${overrideLabel.replace(/_/g, " ")}`
    : approvedUi
      ? "Approved"
      : caseRecord.state.replace("-", " ");
  const traceSteps = useMemo(
    () => mergeTraceSteps(oid || caseRecord.id, liveSteps, httpSteps),
    [caseRecord.id, httpSteps, liveSteps, oid],
  );

  return (
    <>
      {overrideOpen ? (
        <VerdictDialog onClose={() => setOverrideOpen(false)} onSubmit={handleOverrideSubmit} />
      ) : null}

      <div className="grid min-h-full gap-3 p-3 xl:grid-cols-[minmax(0,1fr)_24rem]">
        <div className="min-w-0 space-y-3">
          <section className="rounded-lg border border-border bg-card/80 p-3">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div>
                <div className="mb-2 flex flex-wrap items-center gap-2">
                  <StatusBadge tone={severityTone[caseRecord.severity]}>{caseRecord.severity}</StatusBadge>
                  <StatusBadge tone="medium">{statusLabel}</StatusBadge>
                  <StatusBadge tone="info">{caseRecord.executionPath}</StatusBadge>
                </div>
                <h1 className="text-xl font-semibold tracking-tight">{caseRecord.title}</h1>
                <p className="mt-1 text-sm text-muted-foreground">
                  <span data-customer-data>
                    {caseRecord.entity} · {caseRecord.source}
                  </span>
                </p>
              </div>

              <div className="rounded-md border border-border bg-background/70 px-3 py-2 text-right">
                {triageLoading ? (
                  <div className="flex flex-col items-end gap-1">
                    <LoaderCircle className="h-6 w-6 animate-spin text-primary" aria-hidden />
                    <div className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                      analysing…
                    </div>
                  </div>
                ) : (
                  <>
                    <div className="metric-tabular font-mono text-2xl font-semibold">
                      {Math.round(caseRecord.confidence * 100)}%
                    </div>
                    <div className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                      calibrated confidence
                    </div>
                  </>
                )}
              </div>
            </div>

            {!oid ? null : !overrideApplied && !approvedUi ? (
              <div className="mt-3 flex flex-wrap gap-2 border-t border-border pt-3">
                <button
                  type="button"
                  onClick={handleApprove}
                  className="inline-flex items-center gap-1.5 rounded-md bg-signal-good/10 px-3 py-1.5 text-xs font-medium text-signal-good ring-1 ring-signal-good/30 transition-colors hover:bg-signal-good/20"
                >
                  <ThumbsUp className="h-3.5 w-3.5" />
                  Approve verdict
                </button>
                <button
                  type="button"
                  onClick={() => setOverrideOpen(true)}
                  className="inline-flex items-center gap-1.5 rounded-md bg-severity-medium/10 px-3 py-1.5 text-xs font-medium text-severity-medium ring-1 ring-severity-medium/30 transition-colors hover:bg-severity-medium/20"
                >
                  <ThumbsDown className="h-3.5 w-3.5" />
                  Override verdict
                </button>
              </div>
            ) : null}

            {(overrideApplied || approvedUi) && (
              <div className="mt-3 flex items-start gap-2 rounded-md border border-signal-good/30 bg-signal-good/10 px-3 py-2">
                <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-signal-good" />
                <div className="text-xs">
                  <span className="font-medium text-signal-good">
                    {overrideApplied ? "Override recorded" : "Verdict approved"}
                  </span>
                  <p className="mt-0.5 text-muted-foreground">
                    A new signed attestation envelope has been recorded where applicable. Original
                    verdict preserved in the log.
                  </p>
                </div>
              </div>
            )}
          </section>

          <section className="rounded-lg border border-border bg-card/80">
            <header className="flex items-center gap-2 border-b border-border px-3 py-2">
              <FileSignature className="h-4 w-4 text-muted-foreground" />
              <h2 className="text-sm font-semibold">Reasoning &amp; evidence</h2>
            </header>

            {triageLoading ? (
              /* ── Centered pipeline stepper while triage is in flight ── */
              <TriagePipelineStepper liveSteps={traceSteps} caseId={oid || caseRecord.id} />
            ) : (
              <div className="p-3">
                <InvestigatorTraceStepper
                  liveSteps={traceSteps}
                  executionPath={caseRecord.executionPath}
                  verdict={caseRecord.verdict}
                />
                <ReasoningTraceViewer
                  executionPath={caseRecord.executionPath}
                  featureImpacts={caseRecord.featureImpacts}
                  httpSteps={httpSteps}
                  liveSteps={traceSteps}
                />
              </div>
            )}
          </section>
        </div>

        <aside className="space-y-3">
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
                  <p className="mt-1 text-xs leading-5 text-muted-foreground">
                    <span data-customer-data>{item.value}</span>
                  </p>
                </div>
              ))}
            </div>
          </section>

          <section className="rounded-lg border border-signal-good/30 bg-signal-good/10 p-3">
            <div className="flex items-start gap-2">
              <CheckCircle2 className="mt-0.5 h-4 w-4 text-signal-good" />
              <div>
                <h2 className="text-sm font-semibold text-signal-good">Attestation Valid</h2>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">
                  Classifier envelope signature verified. Analyst overrides create a new signed
                  envelope.
                </p>
              </div>
            </div>
          </section>

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
