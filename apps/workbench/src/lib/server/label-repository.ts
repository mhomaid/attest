import { sql } from "kysely";
import { toAnalystLabel, type AnalystLabel } from "@/lib/labels";
import { db } from "@/lib/db";

let schemaReady: Promise<void> | null = null;

export function ensureLabelSchema(): Promise<void> {
  schemaReady ??= (async () => {
    await sql`
      CREATE TABLE IF NOT EXISTS workbench_analyst_labels (
        id text PRIMARY KEY,
        case_id text NOT NULL,
        envelope_id text NOT NULL,
        agreement text NOT NULL CHECK (agreement IN ('agree', 'disagree')),
        disposition text NOT NULL CHECK (
          disposition IN ('true_positive', 'false_positive', 'benign', 'needs_investigation')
        ),
        reason text NOT NULL,
        user_id text NOT NULL,
        user_email text NOT NULL,
        created_at timestamptz NOT NULL
      )
    `.execute(db);

    await sql`
      CREATE INDEX IF NOT EXISTS workbench_analyst_labels_created_at_idx
      ON workbench_analyst_labels (created_at)
    `.execute(db);

    await sql`
      CREATE INDEX IF NOT EXISTS workbench_analyst_labels_envelope_idx
      ON workbench_analyst_labels (envelope_id)
    `.execute(db);
  })();
  return schemaReady;
}

export async function saveAnalystLabel(row: AnalystLabel): Promise<AnalystLabel> {
  await ensureLabelSchema();
  await db
    .insertInto("workbench_analyst_labels")
    .values({
      id: row.id,
      case_id: row.case_id,
      envelope_id: row.envelope_id,
      agreement: row.agreement,
      disposition: row.disposition,
      reason: row.reason,
      user_id: row.user_id,
      user_email: row.user_email,
      created_at: new Date(row.created_at),
    })
    .execute();
  return row;
}

export async function listAnalystLabels(): Promise<AnalystLabel[]> {
  await ensureLabelSchema();
  const rows = await db
    .selectFrom("workbench_analyst_labels")
    .selectAll()
    .orderBy("created_at", "asc")
    .execute();
  return rows.map((row) => toAnalystLabel(row));
}
