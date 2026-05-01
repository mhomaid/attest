import { sql } from "kysely";
import { db } from "@/lib/db";

export type CaseTraceStepInput = {
  case_id: string;
  agent_action_id: string;
  agent_id: string;
  execution_path: string;
  step_kind: string;
  summary: string;
  ts: number | string | Date;
};

let schemaReady: Promise<void> | null = null;

export function ensureCaseSchema(): Promise<void> {
  schemaReady ??= (async () => {
    await sql`
      CREATE TABLE IF NOT EXISTS workbench_cases (
        case_id text PRIMARY KEY,
        tenant_id text NOT NULL DEFAULT 'default',
        event jsonb NOT NULL,
        baseline jsonb,
        detection jsonb,
        triage_status text NOT NULL DEFAULT 'idle',
        triage_started_at timestamptz,
        triage_completed_at timestamptz,
        verdict jsonb,
        error text,
        created_at timestamptz NOT NULL DEFAULT now(),
        updated_at timestamptz NOT NULL DEFAULT now()
      )
    `.execute(db);

    await sql`
      CREATE TABLE IF NOT EXISTS workbench_case_trace_steps (
        id text PRIMARY KEY,
        case_id text NOT NULL REFERENCES workbench_cases(case_id) ON DELETE CASCADE,
        agent_action_id text NOT NULL,
        agent_id text NOT NULL,
        execution_path text NOT NULL,
        step_kind text NOT NULL,
        summary text NOT NULL,
        ts timestamptz NOT NULL,
        created_at timestamptz NOT NULL DEFAULT now(),
        UNIQUE (case_id, agent_action_id, step_kind, ts)
      )
    `.execute(db);

    await sql`
      CREATE INDEX IF NOT EXISTS workbench_case_trace_steps_case_ts_idx
      ON workbench_case_trace_steps (case_id, ts)
    `.execute(db);
  })();
  return schemaReady;
}

export async function upsertCaseSnapshot(args: {
  caseId: string;
  tenantId?: string;
  event: unknown;
  baseline?: unknown | null;
  detection?: unknown | null;
}) {
  await ensureCaseSchema();
  await db
    .insertInto("workbench_cases")
    .values({
      case_id: args.caseId,
      tenant_id: args.tenantId ?? "default",
      event: args.event,
      baseline: args.baseline ?? null,
      detection: args.detection ?? null,
      triage_status: "idle",
      triage_started_at: null,
      triage_completed_at: null,
      verdict: null,
      error: null,
      created_at: new Date(),
      updated_at: new Date(),
    })
    .onConflict((oc) =>
      oc.column("case_id").doUpdateSet({
        event: args.event,
        baseline: args.baseline ?? null,
        detection: args.detection ?? null,
        updated_at: new Date(),
      }),
    )
    .execute();
}

export async function getCase(caseId: string) {
  await ensureCaseSchema();
  return db
    .selectFrom("workbench_cases")
    .selectAll()
    .where("case_id", "=", caseId)
    .executeTakeFirst();
}

export async function markTriageRunning(caseId: string) {
  await ensureCaseSchema();
  await db
    .updateTable("workbench_cases")
    .set({
      triage_status: "running",
      triage_started_at: new Date(),
      error: null,
      updated_at: new Date(),
    })
    .where("case_id", "=", caseId)
    .execute();
}

export async function markTriageComplete(caseId: string, verdict: unknown) {
  await ensureCaseSchema();
  await db
    .updateTable("workbench_cases")
    .set({
      triage_status: "complete",
      triage_completed_at: new Date(),
      verdict,
      error: null,
      updated_at: new Date(),
    })
    .where("case_id", "=", caseId)
    .execute();
}

export async function markTriageFailed(caseId: string, error: string) {
  await ensureCaseSchema();
  await db
    .updateTable("workbench_cases")
    .set({
      triage_status: "failed",
      error,
      updated_at: new Date(),
    })
    .where("case_id", "=", caseId)
    .execute();
}

export async function appendTraceSteps(caseId: string, steps: CaseTraceStepInput[]) {
  if (steps.length === 0) return;
  await ensureCaseSchema();
  for (const step of steps) {
    const ts = normalizeTraceTimestamp(step.ts);
    await db
      .insertInto("workbench_case_trace_steps")
      .values({
        id: `${caseId}:${step.agent_action_id}:${step.step_kind}:${ts.toISOString()}`,
        case_id: caseId,
        agent_action_id: step.agent_action_id,
        agent_id: step.agent_id,
        execution_path: step.execution_path,
        step_kind: step.step_kind,
        summary: step.summary,
        ts,
        created_at: new Date(),
      })
      .onConflict((oc) => oc.column("id").doNothing())
      .execute();
  }
}

export async function listTraceSteps(caseId: string) {
  await ensureCaseSchema();
  return db
    .selectFrom("workbench_case_trace_steps")
    .selectAll()
    .where("case_id", "=", caseId)
    .orderBy("ts", "asc")
    .execute();
}

function normalizeTraceTimestamp(ts: number | string | Date): Date {
  if (ts instanceof Date) return ts;
  if (typeof ts === "number") return new Date(ts > 10_000_000_000 ? ts : ts * 1000);
  return new Date(ts);
}
