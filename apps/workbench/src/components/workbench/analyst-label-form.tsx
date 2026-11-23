"use client";

import { useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";
import { LABEL_DISPOSITIONS, type Agreement, type LabelDisposition } from "@/lib/labels";
import { cn } from "@/lib/utils";

const ENVELOPE_ID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

const dispositionLabel: Record<LabelDisposition, string> = {
  true_positive: "True positive",
  false_positive: "False positive",
  benign: "Benign",
  needs_investigation: "Needs investigation",
};

/**
 * Records an analyst label against the verdict envelope. The server stores
 * the session user and the time; this form only sends the envelope, the
 * agreement, the correct disposition and a reason. Hidden unless the session
 * has `label.write` (analyst or lead).
 */
export function AnalystLabelForm({
  caseId,
  envelopeId,
}: {
  caseId: string;
  envelopeId?: string;
}) {
  const [allowed, setAllowed] = useState(false);
  const [agreement, setAgreement] = useState<Agreement>("agree");
  const [disposition, setDisposition] = useState<LabelDisposition>("true_positive");
  const [reason, setReason] = useState("");
  const [envelopeDraft, setEnvelopeDraft] = useState<string | null>(null);
  const envelope = envelopeDraft ?? envelopeId ?? "";
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void fetch("/api/auth/me")
      .then((res) => (res.ok ? res.json() : null))
      .then((me: { permissions?: unknown } | null) => {
        if (cancelled) return;
        const permissions = Array.isArray(me?.permissions) ? me.permissions : [];
        setAllowed(permissions.includes("label.write"));
      })
      .catch(() => {
        if (!cancelled) setAllowed(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!allowed) return null;

  const envelopeOk = ENVELOPE_ID.test(envelope.trim());

  const onSubmit = async (event: FormEvent) => {
    event.preventDefault();
    if (!envelopeOk || !reason.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const res = await fetch(`/api/cases/${encodeURIComponent(caseId)}/labels`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          envelope_id: envelope.trim(),
          agreement,
          disposition,
          reason: reason.trim(),
        }),
      });
      const data = (await res.json().catch(() => ({}))) as { error?: unknown; created_at?: unknown };
      if (!res.ok) {
        setError(typeof data.error === "string" ? data.error : "Could not save label");
        return;
      }
      setSavedAt(typeof data.created_at === "string" ? data.created_at : new Date().toISOString());
      setReason("");
      toast.success("Label recorded");
    } catch {
      setError("Could not save label");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="rounded-lg border border-border bg-card/80" data-testid="analyst-label-form">
      <header className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
        <h2 className="text-sm font-semibold">Verdict label</h2>
        <a
          href="/api/labels/export"
          className="text-xs text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
        >
          Export CSV
        </a>
      </header>
      <form onSubmit={onSubmit} className="space-y-3 p-3">
        <p className="text-xs text-muted-foreground">
          Agree or disagree, and record the correct disposition. Your user, the time and this
          envelope id are stored with the label.
        </p>

        <div>
          <span className="mb-1 block text-xs font-medium text-muted-foreground">Envelope</span>
          <input
            value={envelope}
            onChange={(e) => setEnvelopeDraft(e.target.value)}
            spellCheck={false}
            autoComplete="off"
            placeholder="envelope id"
            className="w-full rounded-md border border-border bg-background/60 px-3 py-2 font-mono text-xs text-foreground placeholder:text-muted-foreground/50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30"
          />
        </div>

        <div>
          <span className="mb-1 block text-xs font-medium text-muted-foreground">Agreement</span>
          <div className="grid grid-cols-2 gap-2">
            {(["agree", "disagree"] as const).map((value) => (
              <button
                key={value}
                type="button"
                aria-pressed={agreement === value}
                onClick={() => setAgreement(value)}
                className={cn(
                  "rounded-md border px-3 py-2 text-left text-xs capitalize transition-colors",
                  agreement === value
                    ? "border-primary bg-primary/10 text-primary"
                    : "border-border bg-background/60 text-muted-foreground hover:bg-secondary/50",
                )}
              >
                {value}
              </button>
            ))}
          </div>
        </div>

        <div>
          <span className="mb-1 block text-xs font-medium text-muted-foreground">
            Correct disposition
          </span>
          <div className="grid grid-cols-2 gap-2">
            {LABEL_DISPOSITIONS.map((value) => (
              <button
                key={value}
                type="button"
                aria-pressed={disposition === value}
                onClick={() => setDisposition(value)}
                className={cn(
                  "rounded-md border px-3 py-2 text-left text-xs transition-colors",
                  disposition === value
                    ? "border-primary bg-primary/10 text-primary"
                    : "border-border bg-background/60 text-muted-foreground hover:bg-secondary/50",
                )}
              >
                {dispositionLabel[value]}
              </button>
            ))}
          </div>
        </div>

        <div>
          <label className="mb-1 block text-xs font-medium text-muted-foreground" htmlFor="label-reason">
            Reason <span className="text-destructive">*</span>
          </label>
          <textarea
            id="label-reason"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            rows={3}
            maxLength={2000}
            placeholder="Why this disposition is correct"
            className="w-full resize-none rounded-md border border-border bg-background/60 px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30"
          />
        </div>

        {error ? <p className="text-xs text-destructive">{error}</p> : null}
        {savedAt ? (
          <p className="text-xs text-muted-foreground">Label recorded at {savedAt}.</p>
        ) : null}

        <button
          type="submit"
          disabled={!envelopeOk || !reason.trim() || busy}
          className="rounded-md bg-primary px-3 py-2 text-xs font-medium text-primary-foreground transition-opacity disabled:opacity-40"
        >
          {busy ? "Saving…" : "Record label"}
        </button>
      </form>
    </section>
  );
}
