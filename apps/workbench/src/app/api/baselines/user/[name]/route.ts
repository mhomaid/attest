import { NextRequest, NextResponse } from "next/server";

const CP_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ name: string }> },
) {
  const { name } = await params;
  const upstream = `${CP_URL}/v1/baselines/user/${encodeURIComponent(name)}`;

  try {
    const res = await fetch(upstream, { next: { revalidate: 0 } });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch (err) {
    console.error("[api/baselines/user] upstream error:", err);
    return NextResponse.json({ error: "control-plane unavailable" }, { status: 503 });
  }
}
