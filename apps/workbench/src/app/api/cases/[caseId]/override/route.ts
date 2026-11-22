import { requirePermission } from "@/lib/authz";
import { AUDIENCE, withServiceAuth } from "@/lib/service-auth";
import { NextResponse } from "next/server";

const ORCHESTRATOR_URL =
  process.env.ORCHESTRATOR_URL ?? "http://localhost:4300";

export async function POST(
  req: Request,
  { params }: { params: Promise<{ caseId: string }> },
) {
  const authz = await requirePermission("case.override");
  if (!authz.ok) return authz.response;

  const { caseId } = await params;
  let body: unknown;
  try {
    body = await req.json();
  } catch {
    return NextResponse.json({ error: "invalid JSON" }, { status: 400 });
  }

  try {
    const res = await fetch(
      `${ORCHESTRATOR_URL.replace(/\/$/, "")}/v1/cases/${encodeURIComponent(caseId)}/override`,
      withServiceAuth(AUDIENCE.orchestrator, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Actor-Id": authz.user.id,
          "X-Actor-Email": authz.user.email,
          "X-Actor-Role": authz.role,
        },
        body: JSON.stringify(body),
      }),
    );

    const data = await res.json().catch(() => ({
      error: "invalid JSON from orchestrator",
    }));
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json(
      { error: "orchestrator unavailable" },
      { status: 503 },
    );
  }
}
