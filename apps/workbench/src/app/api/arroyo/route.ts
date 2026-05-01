import { NextResponse } from "next/server";

const ARROYO_URL = process.env.ARROYO_URL ?? "http://localhost:5115";

/**
 * GET /api/arroyo
 * Returns Arroyo pipeline status: list of pipelines with their running-job state.
 */
export async function GET() {
  try {
    const [pipelinesRes] = await Promise.all([
      fetch(`${ARROYO_URL}/api/v1/pipelines`, { cache: "no-store" }),
    ]);

    if (!pipelinesRes.ok) {
      return NextResponse.json({ pipelines: [], error: "arroyo unreachable" }, { status: 200 });
    }

    const body = await pipelinesRes.json();
    const pipelines: { id: string; name: string; state: string }[] = [];

    for (const p of body.data ?? []) {
      // Fetch job state for each pipeline
      try {
        const jobsRes = await fetch(`${ARROYO_URL}/api/v1/pipelines/${p.id}/jobs`, {
          cache: "no-store",
        });
        const jobs = jobsRes.ok ? await jobsRes.json() : { data: [] };
        const latestJob = (jobs.data ?? []).at(-1);
        pipelines.push({
          id: p.id,
          name: p.name,
          state: latestJob?.state ?? "Unknown",
        });
      } catch {
        pipelines.push({ id: p.id, name: p.name, state: "Unknown" });
      }
    }

    return NextResponse.json({ pipelines });
  } catch {
    return NextResponse.json({ pipelines: [], error: "arroyo unreachable" }, { status: 200 });
  }
}
