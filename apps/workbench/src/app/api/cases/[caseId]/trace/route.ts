import { NextResponse } from "next/server";
import { appendTraceSteps, getCase, listTraceSteps } from "@/lib/server/case-repository";

type TracePostBody = {
  steps?: Array<{
    case_id?: string;
    agent_action_id?: string;
    agent_id?: string;
    execution_path?: string;
    step_kind?: string;
    summary?: string;
    ts?: number | string;
  }>;
};

/**
 * Native Next trace endpoint backed by Postgres.
 * This replaces apps/workbench-api and removes file-backed trace reads.
 */
export async function GET(
  _req: Request,
  { params }: { params: Promise<{ caseId: string }> },
) {
  const { caseId } = await params;
  try {
    const rows = await listTraceSteps(caseId);
    let steps = rows.map((row) => ({
      kind: row.step_kind,
      agent_action_id: row.agent_action_id,
      agent_id: row.agent_id,
      execution_path: row.execution_path,
      verdict: row.summary,
      tool_call_count: row.step_kind === "tool_call" ? 1 : 0,
      belief_count: row.step_kind === "intermediate_belief" ? 1 : 0,
    }));

    if (steps.length === 0) {
      const persisted = await getCase(caseId);
      if (persisted?.verdict) {
        const verdict = persisted.verdict as Record<string, unknown>;
        steps = [
          {
            kind: "triage_complete",
            agent_action_id: String(verdict.action_id ?? "unknown"),
            agent_id: "triager-hybrid-v1",
            execution_path: String(verdict.execution_path ?? "unknown"),
            verdict: `${String(verdict.verdict ?? "unknown")} · novelty ${Number(verdict.novelty_score ?? 0).toFixed(3)} · latency ${String(verdict.latency_ms ?? "unknown")}ms`,
            tool_call_count: 0,
            belief_count: 0,
          },
        ];
      }
    }

    return NextResponse.json({ case_id: caseId, steps });
  } catch (err) {
    const message = err instanceof Error ? err.message : "trace read failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function POST(
  req: Request,
  { params }: { params: Promise<{ caseId: string }> },
) {
  const { caseId } = await params;
  const body = (await req.json().catch(() => ({}))) as TracePostBody;
  const rawSteps = body.steps ?? [];
  const steps = rawSteps
    .filter((s) => s.agent_action_id && s.step_kind && s.ts)
    .map((s) => ({
      case_id: caseId,
      agent_action_id: s.agent_action_id!,
      agent_id: s.agent_id ?? "unknown",
      execution_path: s.execution_path ?? "unknown",
      step_kind: s.step_kind!,
      summary: s.summary ?? "",
      ts: s.ts!,
    }));

  await appendTraceSteps(caseId, steps);
  return NextResponse.json({ ok: true, inserted: steps.length });
}
