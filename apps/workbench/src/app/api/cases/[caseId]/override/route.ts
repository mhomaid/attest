import { auth } from "@/lib/auth";
import { headers } from "next/headers";
import { NextResponse } from "next/server";

const ORCHESTRATOR_URL =
  process.env.ORCHESTRATOR_URL ?? "http://localhost:4300";

export async function POST(
  req: Request,
  { params }: { params: Promise<{ caseId: string }> },
) {
  const hdrs = await headers();
  const session = await auth.api.getSession({ headers: hdrs });
  if (!session?.user) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const { caseId } = await params;
  let body: unknown;
  try {
    body = await req.json();
  } catch {
    return NextResponse.json({ error: "invalid JSON" }, { status: 400 });
  }

  const email = session.user.email ?? "";

  try {
    const res = await fetch(
      `${ORCHESTRATOR_URL.replace(/\/$/, "")}/v1/cases/${encodeURIComponent(caseId)}/override`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-Actor-Id": session.user.id,
          "X-Actor-Email": email,
        },
        body: JSON.stringify(body),
      },
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
