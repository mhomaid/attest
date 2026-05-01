import { NextResponse } from "next/server";

const WB_URL = process.env.WORKBENCH_API_URL ?? "http://localhost:4400";

/** Proxies to workbench-api Phase 7 trace endpoint (avoids CORS; uses server-side env). */
export async function GET(
  _req: Request,
  { params }: { params: Promise<{ caseId: string }> },
) {
  const { caseId } = await params;
  try {
    const res = await fetch(`${WB_URL.replace(/\/$/, "")}/v1/cases/${caseId}/trace`, {
      cache: "no-store",
    });
    const data = await res.json().catch(() => ({ error: "invalid JSON from workbench-api" }));
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "workbench-api unavailable" }, { status: 503 });
  }
}
