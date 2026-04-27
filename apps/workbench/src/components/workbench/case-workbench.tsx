import { CheckCircle2, Clock3, Database, FileSignature, ShieldAlert } from "lucide-react";
import { ClassifierEvidencePanel } from "@/components/workbench/classifier-evidence-panel";
import { StatusBadge } from "@/components/workbench/status-badge";
import type { CaseRecord, Severity } from "@/lib/mock-data";

const severityTone: Record<Severity, "critical" | "high" | "medium" | "low"> = {
  critical: "critical",
  high: "high",
  medium: "medium",
  low: "low",
};

export function CaseWorkbench({ caseRecord }: { caseRecord: CaseRecord }) {
  return (
    <div className="grid min-h-full gap-3 p-3 xl:grid-cols-[minmax(0,1fr)_24rem]">
      <div className="min-w-0 space-y-3">
        <section className="rounded-lg border border-border bg-card/80 p-3">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <div className="mb-2 flex flex-wrap items-center gap-2">
                <StatusBadge tone={severityTone[caseRecord.severity]}>{caseRecord.severity}</StatusBadge>
                <StatusBadge tone="medium">{caseRecord.state.replace("-", " ")}</StatusBadge>
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
        </section>

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
              <div key={`${item.time}-${item.actor}`} className="grid gap-3 px-3 py-3 md:grid-cols-[5rem_8rem_minmax(0,1fr)]">
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
                <p className="mt-1 text-xs leading-5 text-muted-foreground">{item.value}</p>
              </div>
            ))}
          </div>
        </section>

        <ClassifierEvidencePanel features={caseRecord.featureImpacts} />

        <section className="rounded-lg border border-signal-good/30 bg-signal-good/10 p-3">
          <div className="flex items-start gap-2">
            <CheckCircle2 className="mt-0.5 h-4 w-4 text-signal-good" />
            <div>
              <h2 className="text-sm font-semibold text-signal-good">Attestation Valid</h2>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">
                Classifier envelope signature verified. Original verdict is immutable; analyst
                overrides will create a new signed envelope.
              </p>
            </div>
          </div>
        </section>

        <section className="rounded-lg border border-severity-medium/30 bg-severity-medium/10 p-3">
          <div className="flex items-start gap-2">
            <ShieldAlert className="mt-0.5 h-4 w-4 text-severity-medium" />
            <div>
              <h2 className="text-sm font-semibold text-severity-medium">Edge State Preview</h2>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">
                Calibration is six days old. The UI already reserves space for stale calibration
                and degraded-provider warnings.
              </p>
            </div>
          </div>
        </section>
      </aside>
    </div>
  );
}
