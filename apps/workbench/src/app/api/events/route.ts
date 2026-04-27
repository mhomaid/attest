import { NextRequest, NextResponse } from "next/server";

const CP_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";

export async function GET(req: NextRequest) {
  const id = req.nextUrl.searchParams.get("id");
  const upstream = id
    ? `${CP_URL}/v1/events/recent?id=${encodeURIComponent(id)}`
    : `${CP_URL}/v1/events/recent`;

  try {
    const res = await fetch(upstream, {
      next: { revalidate: 0 }, // always fresh
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch (err) {
    console.error("[api/events] upstream error:", err);
    return NextResponse.json({ error: "control-plane unavailable" }, { status: 503 });
  }
}
