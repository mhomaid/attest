import { NextResponse } from "next/server";

const LOAD_GEN_URL = process.env.LOAD_GEN_URL ?? "http://localhost:9100";

export async function POST() {
  try {
    const res = await fetch(`${LOAD_GEN_URL}/stop`, { method: "POST" });
    const data = await res.json().catch(() => ({}));
    return NextResponse.json(data, { status: res.status });
  } catch (e) {
    return NextResponse.json({ error: String(e) }, { status: 502 });
  }
}
