import { parseCaseId, parseLabelBody, type AnalystLabel } from "@/lib/labels";
import { requirePermission } from "@/lib/authz";
import { saveAnalystLabel } from "@/lib/server/label-repository";
import { NextResponse } from "next/server";

/**
 * POST /api/cases/:caseId/labels
 * Analyst or lead records agree/disagree, the correct disposition and a reason.
 * User and time come from the session, not the body. Envelope id is required.
 */
export async function POST(
  req: Request,
  { params }: { params: Promise<{ caseId: string }> },
) {
  const authz = await requirePermission("label.write");
  if (!authz.ok) return authz.response;

  const { caseId } = await params;
  const parsedCaseId = parseCaseId(caseId);
  if (!parsedCaseId) {
    return NextResponse.json({ error: "invalid case id" }, { status: 400 });
  }

  let body: unknown;
  try {
    body = await req.json();
  } catch {
    return NextResponse.json({ error: "invalid JSON" }, { status: 400 });
  }

  const parsed = parseLabelBody(body);
  if (!parsed.ok) {
    return NextResponse.json({ error: parsed.error }, { status: 400 });
  }

  const row: AnalystLabel = {
    id: crypto.randomUUID(),
    case_id: parsedCaseId,
    envelope_id: parsed.value.envelope_id,
    agreement: parsed.value.agreement,
    disposition: parsed.value.disposition,
    reason: parsed.value.reason,
    user_id: authz.user.id,
    user_email: authz.user.email,
    created_at: new Date().toISOString(),
  };

  try {
    await saveAnalystLabel(row);
  } catch {
    return NextResponse.json({ error: "label store unavailable" }, { status: 503 });
  }

  return NextResponse.json(row, { status: 201 });
}
