import { labelsToCsv } from "@/lib/labels";
import { requirePermission } from "@/lib/authz";
import { listAnalystLabels } from "@/lib/server/label-repository";
import { NextResponse } from "next/server";

/**
 * GET /api/labels/export?format=csv
 * Training export. Parquet is not produced here; callers that pass another
 * format get 400. Same role gate as recording a label.
 */
export async function GET(req: Request) {
  const authz = await requirePermission("label.write");
  if (!authz.ok) return authz.response;

  const format = new URL(req.url).searchParams.get("format") ?? "csv";
  if (format !== "csv") {
    return NextResponse.json({ error: "unsupported format" }, { status: 400 });
  }

  try {
    const rows = await listAnalystLabels();
    const csv = labelsToCsv(rows);
    return new NextResponse(csv, {
      status: 200,
      headers: {
        "Content-Type": "text/csv; charset=utf-8",
        "Content-Disposition": 'attachment; filename="analyst-labels.csv"',
        "Cache-Control": "no-store",
      },
    });
  } catch {
    return NextResponse.json({ error: "label store unavailable" }, { status: 503 });
  }
}
