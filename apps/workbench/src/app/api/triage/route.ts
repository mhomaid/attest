import { NextRequest, NextResponse } from "next/server";

const ORCHESTRATOR_URL =
  process.env.ORCHESTRATOR_URL ?? "http://localhost:4300";

export async function POST(req: NextRequest) {
  try {
    const body = await req.json();
    const res = await fetch(`${ORCHESTRATOR_URL}/triage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(10_000),
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch (err) {
    return NextResponse.json(
      { error: String(err) },
      { status: 503 },
    );
  }
}
