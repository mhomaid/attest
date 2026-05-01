import { NextRequest } from "next/server";
import {
  getCase,
  markTriageComplete,
  markTriageFailed,
  markTriageRunning,
  upsertCaseSnapshot,
} from "@/lib/server/case-repository";

const ORCHESTRATOR_URL =
  process.env.ORCHESTRATOR_URL ?? "http://localhost:4300";
const CP_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";

// Dev/runtime-local idempotency cache. This prevents browser refreshes or
// duplicate client mounts from creating multiple LLM investigations for the
// same case in the same Next.js process.
const TRIAGE_RESULT_CACHE = new Map<string, unknown>();
const TRIAGE_IN_FLIGHT = new Map<string, Promise<unknown>>();

/** Severity string → 0-1 score (mirrors the Rust feature extractor). */
function severityScore(s: string): number {
  switch (s.toLowerCase()) {
    case "informational":
      return 0.1;
    case "low":
      return 0.3;
    case "medium":
      return 0.5;
    case "high":
      return 0.8;
    case "critical":
    case "fatal":
      return 1.0;
    default:
      return 0.5;
  }
}

type FeatureDefaults = Partial<
  Record<
    | "severity_score"
    | "entity_reputation_score"
    | "baseline_deviation"
    | "threat_intel_hit_count"
    | "asset_criticality"
    | "prior_disposition_ratio",
    number
  >
>;

function hasNumericFeature(
  alert: Record<string, unknown>,
  key: string,
): boolean {
  return typeof alert[key] === "number" && Number.isFinite(alert[key]);
}

function setFeatureDefault(
  alert: Record<string, unknown>,
  key: keyof FeatureDefaults,
  value: number | undefined,
) {
  if (value === undefined || hasNumericFeature(alert, key)) return;
  alert[key] = value;
}

function applyOperationFeatureDefaults(alert: Record<string, unknown>) {
  const operation = String(
    alert.api_operation ?? alert.eventName ?? "",
  ).toLowerCase();
  const service = String(
    alert.api_service ?? alert.eventSource ?? "",
  ).toLowerCase();
  const actor = String(alert.actor_user_name ?? "").toLowerCase();

  const defaults: FeatureDefaults = {};

  if (
    operation.includes("putbucketacl") ||
    operation.includes("putbucketpolicy")
  ) {
    defaults.severity_score = 0.85;
    defaults.baseline_deviation = 0.7;
    defaults.threat_intel_hit_count = 1;
    defaults.asset_criticality = 0.9;
    defaults.prior_disposition_ratio = 0.8;
  } else if (operation.includes("createaccesskey") || actor === "root") {
    defaults.severity_score = 1.0;
    defaults.baseline_deviation = 0.9;
    defaults.asset_criticality = 1.0;
    defaults.prior_disposition_ratio = 0.9;
  } else if (operation.includes("consolelogin")) {
    defaults.asset_criticality = 0.5;
    defaults.prior_disposition_ratio = 0.35;
  } else if (service.includes("okta") || operation.includes("session.start")) {
    defaults.severity_score = 0.75;
    defaults.entity_reputation_score = 0.6;
    defaults.baseline_deviation = 0.6;
    defaults.threat_intel_hit_count = 1;
    defaults.asset_criticality = 0.5;
    defaults.prior_disposition_ratio = 0.7;
  }

  for (const [key, value] of Object.entries(defaults) as Array<
    [keyof FeatureDefaults, number]
  >) {
    setFeatureDefault(alert, key, value);
  }
}

/** Fetch the baseline for a user and compute a simple region-deviation score (0-1). */
async function baselineDeviation(
  username: string,
  region: string,
): Promise<number> {
  try {
    const res = await fetch(
      `${CP_URL}/v1/baselines/user/${encodeURIComponent(username)}`,
      { next: { revalidate: 30 } },
    );
    if (!res.ok) return 0;
    const baseline = (await res.json()) as { regions_seen_30d?: string[] };
    const seen = baseline.regions_seen_30d ?? [];
    if (seen.length === 0) return 0.5; // no history — moderately novel
    return seen.includes(region) ? 0.0 : 1.0; // unseen region = max deviation
  } catch {
    return 0;
  }
}

/**
 * GET /api/cases/[caseId]/triage
 *
 * Durable triage status from Postgres. The browser uses this as source of
 * truth before deciding whether to show "Analyzing" or POST a new triage job.
 */
export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ caseId: string }> },
) {
  const { caseId } = await params;
  const persisted = await getCase(caseId);
  if (!persisted) {
    return Response.json({ status: "missing", verdict: null });
  }

  return Response.json({
    status: persisted.triage_status,
    verdict: persisted.verdict,
    error: persisted.error,
    triage_started_at: persisted.triage_started_at,
    triage_completed_at: persisted.triage_completed_at,
  });
}

/**
 * POST /api/cases/[caseId]/triage
 *
 * Client-side triage proxy — keeps the orchestrator URL server-only and allows
 * the browser to hold a long-lived fetch without a hard timeout.
 * Body: { alert: OcsfEvent }
 */
export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ caseId: string }> },
) {
  const { caseId } = await params;

  let body: { alert?: unknown };
  try {
    body = (await req.json()) as { alert?: unknown };
  } catch {
    return Response.json({ error: "Invalid JSON body" }, { status: 400 });
  }

  if (!body.alert) {
    return Response.json({ error: "Missing alert field" }, { status: 400 });
  }

  const persisted = await getCase(caseId);
  if (persisted?.triage_status === "complete" && persisted.verdict) {
    return Response.json(persisted.verdict);
  }

  const cached = TRIAGE_RESULT_CACHE.get(caseId);
  if (cached) {
    return Response.json(cached);
  }

  const inFlight = TRIAGE_IN_FLIGHT.get(caseId);
  if (inFlight) {
    try {
      return Response.json(await inFlight);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Triage request failed";
      return Response.json({ error: message }, { status: 502 });
    }
  }

  // Enrich the OCSF flat event with pre-computed feature fields so the
  // orchestrator's feature extractor produces meaningful non-zero values.
  const alert = body.alert as Record<string, unknown>;
  const enrichedAlert: Record<string, unknown> = { ...alert };
  applyOperationFeatureDefaults(enrichedAlert);

  // severity_score
  if (typeof enrichedAlert.severity === "string") {
    const score = severityScore(enrichedAlert.severity);
    enrichedAlert.severity_score = Math.max(
      hasNumericFeature(enrichedAlert, "severity_score")
        ? Number(enrichedAlert.severity_score)
        : 0,
      score,
    );
  }
  // source_class_id (class_uid is a string like "3002")
  if (
    !hasNumericFeature(enrichedAlert, "source_class_id") &&
    typeof enrichedAlert.class_uid === "string"
  ) {
    enrichedAlert.source_class_id =
      parseFloat(enrichedAlert.class_uid) || 6003.0;
  }
  // hour_of_day
  if (
    !hasNumericFeature(enrichedAlert, "hour_of_day") &&
    typeof enrichedAlert.time === "string"
  ) {
    const h = enrichedAlert.time.slice(11, 13);
    enrichedAlert.hour_of_day = parseFloat(h) || 12.0;
  }
  // baseline_deviation — fetch baseline and check if region was seen before
  if (
    !enrichedAlert.baseline_deviation &&
    typeof enrichedAlert.actor_user_name === "string" &&
    typeof enrichedAlert.cloud_region === "string" &&
    enrichedAlert.cloud_region !== "unknown"
  ) {
    enrichedAlert.baseline_deviation = await baselineDeviation(
      enrichedAlert.actor_user_name,
      enrichedAlert.cloud_region,
    );
  }

  await upsertCaseSnapshot({
    caseId,
    event: enrichedAlert,
    baseline: null,
  });

  const run = (async () => {
    await markTriageRunning(caseId);
    const upstream = await fetch(`${ORCHESTRATOR_URL}/triage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        case_id: caseId,
        tenant_id: "default",
        alert: enrichedAlert,
      }),
      // No AbortSignal — let the orchestrator take as long as it needs.
      // The browser connection is the natural boundary.
    });

    if (!upstream.ok) {
      throw new Error(`Orchestrator returned ${upstream.status}`);
    }

    return upstream.json() as Promise<unknown>;
  })();

  TRIAGE_IN_FLIGHT.set(caseId, run);

  try {
    const data = await run;
    await markTriageComplete(caseId, data);
    TRIAGE_RESULT_CACHE.set(caseId, data);
    return Response.json(data);
  } catch (err) {
    const message =
      err instanceof Error ? err.message : "Triage request failed";
    await markTriageFailed(caseId, message);
    return Response.json({ error: message }, { status: 502 });
  } finally {
    TRIAGE_IN_FLIGHT.delete(caseId);
  }
}
