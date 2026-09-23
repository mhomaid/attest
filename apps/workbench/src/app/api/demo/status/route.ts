import { NextResponse } from "next/server";

const COLLECTOR_URL = process.env.COLLECTOR_URL ?? "http://localhost:4000";
const ORCHESTRATOR_URL = process.env.ORCHESTRATOR_URL ?? "http://localhost:4300";

export const dynamic = "force-dynamic";

async function up(url: string): Promise<boolean> {
  try {
    const r = await fetch(url, { signal: AbortSignal.timeout(1_500), cache: "no-store" });
    return r.ok;
  } catch {
    return false;
  }
}

export async function GET() {
  const [collector, orchestrator] = await Promise.all([
    up(`${COLLECTOR_URL}/healthz`),
    up(`${ORCHESTRATOR_URL}/healthz`),
  ]);
  return NextResponse.json(
    { live: collector && orchestrator, collector, orchestrator },
    { headers: { "Cache-Control": "no-store" } },
  );
}
