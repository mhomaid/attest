import { NextResponse } from "next/server";

const LOAD_GEN_URL = process.env.LOAD_GEN_URL ?? "http://localhost:9100";

export async function GET() {
  try {
    const res = await fetch(`${LOAD_GEN_URL}/status`, { next: { revalidate: 0 } });
    const data = await res.json().catch(() => ({}));
    return NextResponse.json(data, { status: res.status });
  } catch (e) {
    return NextResponse.json({ error: String(e), running: false }, { status: 502 });
  }
}
