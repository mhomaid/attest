/**
 * Analyst verdict labels. The route stores the signed-in user and the time;
 * callers only supply the envelope, the agreement, the correct disposition
 * and a reason.
 */

export const AGREEMENTS = ["agree", "disagree"] as const;
export type Agreement = (typeof AGREEMENTS)[number];

/** Matches the attestation `Verdict` names, snake_case. */
export const LABEL_DISPOSITIONS = [
  "true_positive",
  "false_positive",
  "benign",
  "needs_investigation",
] as const;
export type LabelDisposition = (typeof LABEL_DISPOSITIONS)[number];

const ENVELOPE_ID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const CASE_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const MAX_REASON = 2000;

export type LabelInput = {
  envelope_id: string;
  agreement: Agreement;
  disposition: LabelDisposition;
  reason: string;
};

export type AnalystLabel = {
  id: string;
  case_id: string;
  envelope_id: string;
  agreement: Agreement;
  disposition: LabelDisposition;
  reason: string;
  user_id: string;
  user_email: string;
  created_at: string;
};

export const LABEL_CSV_HEADER = [
  "envelope_id",
  "case_id",
  "agreement",
  "disposition",
  "reason",
  "user_id",
  "user_email",
  "created_at",
] as const;

export function parseCaseId(value: string): string | undefined {
  return CASE_ID.test(value) ? value : undefined;
}

export function parseLabelBody(
  body: unknown,
): { ok: true; value: LabelInput } | { ok: false; error: string } {
  if (typeof body !== "object" || body === null) {
    return { ok: false, error: "invalid body" };
  }
  const record = body as Record<string, unknown>;
  const envelope = typeof record.envelope_id === "string" ? record.envelope_id.trim() : "";
  if (!ENVELOPE_ID.test(envelope)) return { ok: false, error: "invalid envelope_id" };

  const agreement = record.agreement;
  if (agreement !== "agree" && agreement !== "disagree") {
    return { ok: false, error: "invalid agreement" };
  }

  const disposition = record.disposition;
  if (
    typeof disposition !== "string" ||
    !(LABEL_DISPOSITIONS as readonly string[]).includes(disposition)
  ) {
    return { ok: false, error: "invalid disposition" };
  }

  const reason = typeof record.reason === "string" ? record.reason.trim() : "";
  if (reason.length < 1 || reason.length > MAX_REASON) {
    return { ok: false, error: "invalid reason" };
  }

  return {
    ok: true,
    value: {
      envelope_id: envelope.toLowerCase(),
      agreement,
      disposition: disposition as LabelDisposition,
      reason,
    },
  };
}

function csvField(value: string): string {
  return `"${value.replaceAll('"', '""')}"`;
}

/** RFC-style CSV. Every value is quoted so reasons can contain commas and quotes. */
export function labelsToCsv(rows: readonly AnalystLabel[]): string {
  const lines = [LABEL_CSV_HEADER.join(",")];
  for (const row of rows) {
    lines.push(LABEL_CSV_HEADER.map((key) => csvField(row[key])).join(","));
  }
  return `${lines.join("\n")}\n`;
}

export function toAnalystLabel(row: {
  id: string;
  case_id: string;
  envelope_id: string;
  agreement: string;
  disposition: string;
  reason: string;
  user_id: string;
  user_email: string;
  created_at: Date | string;
}): AnalystLabel {
  const parsed = parseLabelBody({
    envelope_id: row.envelope_id,
    agreement: row.agreement,
    disposition: row.disposition,
    reason: row.reason,
  });
  if (!parsed.ok) throw new Error("stored label failed validation");
  const created =
    row.created_at instanceof Date ? row.created_at : new Date(row.created_at);
  if (Number.isNaN(created.getTime())) throw new Error("stored label has no time");
  return {
    id: row.id,
    case_id: row.case_id,
    envelope_id: parsed.value.envelope_id,
    agreement: parsed.value.agreement,
    disposition: parsed.value.disposition,
    reason: parsed.value.reason,
    user_id: row.user_id,
    user_email: row.user_email,
    created_at: created.toISOString(),
  };
}
