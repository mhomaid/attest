import { NextRequest, NextResponse } from "next/server";
import { getScenario } from "@/lib/scenarios";

const COLLECTOR_URL = process.env.COLLECTOR_URL ?? "http://localhost:4000";
const ORCHESTRATOR_URL = process.env.ORCHESTRATOR_URL ?? "http://localhost:4300";
const DEMO_TENANT = "demo";
const DEMO_SCENARIO = "geo_anomaly";

export type DemoRunResponse = {
  ok: boolean;
  error?: string;
  event_ids: string[];
  action_id: string;
  verdict: string;
  execution_path: string;
  tenant_id: string;
  envelope?: {
    agent_action_id: string;
    case_id: string;
    tenant_id: string;
    verdict: string;
    execution_path: string;
    prev_hash: string;
    signature: string;
    signed_at: string;
  };
  verify?: {
    ok: boolean;
    passed: number;
    total: number;
    verifying_key: string;
    durable: boolean;
    results: { action_id: string; ok: boolean; detail: string }[];
  };
  collector: { ok: boolean; error?: string };
  orchestrator: { ok: boolean; error?: string };
};

// Unauthenticated endpoint: every call signs and appends an envelope, so cap it.
// In-memory is fine at one workbench replica; move to Redis/Postgres if that changes.
const WINDOW_MS = 60_000;
const PER_IP_LIMIT = 5;
const GLOBAL_LIMIT = 60;
const hits = new Map<string, number[]>();

function rateLimited(ip: string): boolean {
  const now = Date.now();
  const recent = (key: string) => (hits.get(key) ?? []).filter((t) => now - t < WINDOW_MS);
  const mine = recent(ip);
  const all = recent("*");
  if (mine.length >= PER_IP_LIMIT || all.length >= GLOBAL_LIMIT) return true;
  hits.set(ip, [...mine, now]);
  hits.set("*", [...all, now]);
  if (hits.size > 10_000) hits.clear();
  return false;
}

export async function POST(req: NextRequest) {
  const ip = req.headers.get("x-forwarded-for")?.split(",")[0]?.trim() || "unknown";
  if (rateLimited(ip)) {
    const limited: DemoRunResponse = {
      ok: false,
      error: "Demo is rate limited. Try again in a minute.",
      event_ids: [],
      action_id: "",
      verdict: "unknown",
      execution_path: "classifier",
      tenant_id: DEMO_TENANT,
      collector: { ok: true },
      orchestrator: { ok: false, error: "rate limited" },
    };
    return NextResponse.json(limited, { status: 429 });
  }

  let scenarioId = DEMO_SCENARIO;
  try {
    const body = (await req.json()) as { scenario_id?: string };
    if (body.scenario_id) scenarioId = body.scenario_id;
  } catch {
    // empty body is fine
  }

  const scenario = getScenario(scenarioId);
  if (!scenario) {
    return NextResponse.json(
      {
        ok: false,
        error: `Unknown scenario: ${scenarioId}`,
        event_ids: [],
        action_id: "",
        verdict: "unknown",
        execution_path: "classifier",
        tenant_id: DEMO_TENANT,
        collector: { ok: false },
        orchestrator: { ok: false },
      } satisfies DemoRunResponse,
      { status: 400 },
    );
  }

  const params = scenario.defaultParams;
  const out: DemoRunResponse = {
    ok: false,
    event_ids: [],
    action_id: "",
    verdict: "unknown",
    execution_path: "classifier",
    tenant_id: DEMO_TENANT,
    collector: { ok: false },
    orchestrator: { ok: false },
  };

  try {
    const r = await fetch(`${COLLECTOR_URL}/ingest`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Tenant-Id": DEMO_TENANT,
      },
      body: JSON.stringify(scenario.buildCloudTrail(params)),
      signal: AbortSignal.timeout(8_000),
    });
    const data = (await r.json()) as { event_ids?: string[]; error?: string };
    if (!r.ok || data.error) throw new Error(data.error ?? `HTTP ${r.status}`);
    out.event_ids = data.event_ids ?? [];
    out.collector = { ok: true };
  } catch (e) {
    out.collector = { ok: false, error: friendlyFetchError(e, "Collector") };
  }

  try {
    const r = await fetch(`${ORCHESTRATOR_URL}/triage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        tenant_id: DEMO_TENANT,
        alert: scenario.buildAlert(params),
      }),
      signal: AbortSignal.timeout(12_000),
    });
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    const triage = (await r.json()) as {
      action_id?: string;
      verdict?: string;
      execution_path?: string;
    };
    out.action_id = triage.action_id ?? "";
    out.verdict = triage.verdict ?? "unknown";
    out.execution_path = triage.execution_path ?? "classifier";
    out.orchestrator = { ok: true };
  } catch (e) {
    out.orchestrator = { ok: false, error: friendlyFetchError(e, "Orchestrator") };
    out.error = "Orchestrator is not reachable. The hosted demo needs the triage service up.";
    return NextResponse.json(out, { status: 503 });
  }

  if (out.action_id) {
    try {
      const [envRes, verifyRes] = await Promise.all([
        fetch(`${ORCHESTRATOR_URL}/v1/attestations/${out.action_id}`, {
          signal: AbortSignal.timeout(5_000),
        }),
        fetch(`${ORCHESTRATOR_URL}/v1/attestations/verify`, {
          signal: AbortSignal.timeout(5_000),
        }),
      ]);
      if (envRes.ok) {
        const env = (await envRes.json()) as DemoRunResponse["envelope"] & {
          agent_action_id?: string;
        };
        out.envelope = {
          agent_action_id: env.agent_action_id ?? out.action_id,
          case_id: env.case_id ?? "",
          tenant_id: env.tenant_id ?? DEMO_TENANT,
          verdict: env.verdict ?? out.verdict,
          execution_path: env.execution_path ?? out.execution_path,
          prev_hash: env.prev_hash ?? "",
          signature: env.signature ?? "",
          signed_at: env.signed_at ?? "",
        };
      }
      if (verifyRes.ok) {
        const report = (await verifyRes.json()) as NonNullable<DemoRunResponse["verify"]>;
        out.verify = { ...report, results: report.results.slice(-5) };
      }
    } catch (e) {
      out.error = `Signed, but verify API failed: ${e}`;
    }
  }

  out.ok = Boolean(out.orchestrator.ok && out.envelope && out.verify?.ok);
  if (!out.ok && !out.error) {
    out.error = out.verify
      ? "Envelope signed, but the log did not verify."
      : "Envelope signed, but the verify API is not available on this orchestrator.";
  }

  return NextResponse.json(out);
}

function friendlyFetchError(e: unknown, service: string): string {
  const raw = String(e);
  if (raw.includes("fetch failed") || raw.includes("ECONNREFUSED") || raw.includes("AbortError")) {
    return `${service} unreachable`;
  }
  return raw;
}
