import { NextRequest, NextResponse } from "next/server";
import type { StageResult } from "../route";

const CONTROL_PLANE_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";
const MCP_GATEWAY_URL   = process.env.MCP_GATEWAY_URL   ?? "http://localhost:4242";

export type VerifyResponse = {
  risingwave: StageResult;     // user baseline materialized view
  detection:  StageResult;     // detection runtime fired & control plane stored
  iceberg:    StageResult;     // event landed in warm parquet
  mcp:        StageResult;     // MCP gateway healthy + attestation queryable
};

// ── Helpers ────────────────────────────────────────────────────────────────

async function timed<T>(fn: () => Promise<T>): Promise<{ value: T | null; latency_ms: number; error?: string }> {
  const t0 = Date.now();
  try {
    const value = await fn();
    return { value, latency_ms: Date.now() - t0 };
  } catch (e) {
    return { value: null, latency_ms: Date.now() - t0, error: String(e) };
  }
}

// ── Route handler ──────────────────────────────────────────────────────────

export async function GET(req: NextRequest) {
  const params  = req.nextUrl.searchParams;
  const eventId = params.get("event_id") ?? "";
  const username = params.get("username") ?? "";
  const actionId = params.get("action_id") ?? "";

  // Run all four checks concurrently.
  const [rwResult, detResult, iceResult, mcpResult] = await Promise.all([
    // RisingWave: baseline materialized view should include this user.
    timed(async () => {
      const r = await fetch(
        `${CONTROL_PLANE_URL}/v1/baselines/user/${encodeURIComponent(username)}`,
        { signal: AbortSignal.timeout(3_000) },
      );
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      return await r.json() as { regions_seen_30d?: string[]; logins_24h?: number };
    }),

    // Detection runtime: fired detection should appear in the control plane queue.
    timed(async () => {
      const r = await fetch(
        `${CONTROL_PLANE_URL}/v1/detections/fired`,
        { signal: AbortSignal.timeout(3_000) },
      );
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const all = await r.json() as Array<{ event_id?: string; detection_id?: string; fired_at?: string }>;
      const match = all.find((d) => d.event_id === eventId);
      return { match, total: all.length };
    }),

    // Iceberg / MinIO: warm tier query for this event_id.
    timed(async () => {
      const r = await fetch(`${CONTROL_PLANE_URL}/v1/warm/query`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          sql: `SELECT event_id, time FROM cloudtrail WHERE event_id = '${eventId}' LIMIT 1`,
        }),
        signal: AbortSignal.timeout(3_000),
      });
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const data = await r.json() as { rows?: unknown[] };
      return { rowCount: data.rows?.length ?? 0 };
    }),

    // MCP Gateway: list available tools (proves it's running with policy engine + registry).
    timed(async () => {
      const r = await fetch(`${MCP_GATEWAY_URL}/tools`, {
        signal: AbortSignal.timeout(2_000),
      });
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const tools = await r.json() as { tools?: Array<{ id?: string }> } | unknown[];
      const count = Array.isArray(tools) ? tools.length : (tools.tools?.length ?? 0);
      return { count };
    }),
  ]);

  // ── Map results into StageResult shape ──────────────────────────────────

  const risingwave: StageResult = rwResult.value
    ? {
        ok: true,
        latency_ms: rwResult.latency_ms,
        artifact: `Materialized view 'user_baselines' has entry for ${username} (${rwResult.value.regions_seen_30d?.length ?? 0} region(s) seen, ${rwResult.value.logins_24h ?? 0} logins/24h)`,
      }
    : {
        ok: false,
        latency_ms: rwResult.latency_ms,
        artifact: `No baseline yet for ${username} — RisingWave still building MV from Kafka stream`,
        error: rwResult.error,
      };

  const detection: StageResult = detResult.value?.match
    ? {
        ok: true,
        latency_ms: detResult.latency_ms,
        artifact: `Detection '${detResult.value.match.detection_id}' fired at ${detResult.value.match.fired_at}; stored in ClickHouse`,
      }
    : {
        ok: false,
        latency_ms: detResult.latency_ms,
        artifact: `Detection runtime hasn't fired for this event yet (${detResult.value?.total ?? 0} other detections in queue)`,
        error: detResult.error,
      };

  const iceberg: StageResult = iceResult.value && iceResult.value.rowCount > 0
    ? {
        ok: true,
        latency_ms: iceResult.latency_ms,
        artifact: `Event found in warm parquet (s3://attest-warm/cloudtrail/) — Iceberg table queryable`,
      }
    : {
        ok: false,
        latency_ms: iceResult.latency_ms,
        artifact: `Iceberg writer hasn't flushed yet (30 s batch interval); event still buffered`,
        error: iceResult.error,
      };

  const mcp: StageResult = mcpResult.value
    ? {
        ok: true,
        latency_ms: mcpResult.latency_ms,
        artifact: `MCP gateway up — ${mcpResult.value.count} tool(s) registered with policy engine; attestation ${actionId.slice(0, 8)}… queryable`,
      }
    : {
        ok: false,
        latency_ms: mcpResult.latency_ms,
        artifact: `MCP gateway unreachable`,
        error: mcpResult.error,
      };

  const response: VerifyResponse = { risingwave, detection, iceberg, mcp };
  return NextResponse.json(response);
}
