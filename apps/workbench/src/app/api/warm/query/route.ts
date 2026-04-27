import { NextRequest, NextResponse } from "next/server";

const CP_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";

export async function POST(req: NextRequest) {
  let body: unknown;
  try {
    body = await req.json();
  } catch {
    return NextResponse.json({ error: "invalid JSON body" }, { status: 400 });
  }

  try {
    const res = await fetch(`${CP_URL}/v1/warm/query`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      next: { revalidate: 0 },
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch (err) {
    console.error("[api/warm/query] upstream error:", err);
    return NextResponse.json({ error: "control-plane unavailable" }, { status: 503 });
  }
}
