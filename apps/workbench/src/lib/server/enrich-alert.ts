import { getCases } from "@/lib/server/case-repository";
import type { Alert } from "@/lib/mock-data";

/**
 * Merges any persisted Workbench-case snapshot fields (triage status,
 * verdict, confidence, OCSF event details) onto a bare HELIQL detection.
 * Returns the original alert untouched when there is no persisted case.
 */
export function mergePersistedCase(
  alert: Alert,
  persisted: Record<string, unknown>,
): Alert {
  const verdict = (persisted.verdict ?? null) as Record<string, unknown> | null;
  const event = (persisted.event ?? null) as Record<string, unknown> | null;
  const status = String(persisted.triage_status ?? "idle");
  const rawVerdict = String(verdict?.verdict ?? alert.verdict);
  const confidence = numberField(
    verdict?.calibrated_confidence,
    alert.confidence,
  );
  const executionPath = normalizeExecutionPath(
    verdict?.execution_path,
    alert.executionPath,
  );
  const toolsUsed = Array.isArray(
    (verdict?.investigation as Record<string, unknown> | undefined)
      ?.evidence_citations,
  )
    ? (
        (verdict?.investigation as Record<string, unknown>)
          .evidence_citations as unknown[]
      ).length
    : 0;

  return {
    ...alert,
    title: formatQueueTitle(alert.title, event),
    source: event?.api_service ? String(event.api_service) : alert.source,
    entity: event?.actor_user_name
      ? String(event.actor_user_name)
      : alert.entity,
    region: event?.cloud_region ? String(event.cloud_region) : alert.region,
    verdict: normalizeVerdict(rawVerdict),
    confidence,
    executionPath,
    state: status === "running" ? "in-flight" : alert.state,
    summary: buildQueueSummary(alert, event, verdict),
    agentStatus: {
      agent: executionPath === "llm" ? "Investigator" : "Triager",
      status:
        status === "complete"
          ? "complete"
          : status === "running"
            ? "running"
            : "idle",
      step:
        status === "complete"
          ? `${rawVerdict.replace(/_/g, " ")} verdict`
          : status === "running"
            ? "Running triage"
            : "Awaiting triage",
      toolsUsed,
    },
  };
}

/**
 * Single batched DB call: enriches every alert in `alerts` with its persisted
 * Workbench case snapshot if one exists. Replaces N×getCase round-trips that
 * used to saturate the connection pool.
 */
export async function enrichAlertsWithPersisted(
  alerts: Alert[],
): Promise<Alert[]> {
  if (alerts.length === 0) return alerts;
  const caseIds = alerts.map((a) => a.caseId);
  const persistedMap = await getCases(caseIds).catch(
    () => new Map<string, unknown>(),
  );
  return alerts.map((alert) => {
    const persisted = persistedMap.get(alert.caseId) as
      | Record<string, unknown>
      | undefined;
    return persisted?.event ? mergePersistedCase(alert, persisted) : alert;
  });
}

function numberField(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function normalizeExecutionPath(
  value: unknown,
  fallback: Alert["executionPath"],
): Alert["executionPath"] {
  return value === "classifier" || value === "llm" || value === "hybrid"
    ? value
    : fallback;
}

function normalizeVerdict(value: string): Alert["verdict"] {
  if (value === "false_positive" || value === "benign") return "benign";
  if (value === "true_positive" || value === "malicious") return "malicious";
  if (value === "needs_investigation" || value === "suspicious")
    return "suspicious";
  return "investigating";
}

function formatQueueTitle(
  fallback: string,
  event: Record<string, unknown> | null,
): string {
  if (!event?.api_operation) return fallback;
  return String(event.api_operation).replace(/([a-z])([A-Z])/g, "$1 $2");
}

function buildQueueSummary(
  alert: Alert,
  event: Record<string, unknown> | null,
  verdict: Record<string, unknown> | null,
): string {
  const parts = [
    event?.api_service ? `service ${event.api_service}` : null,
    event?.cloud_region ? `region ${event.cloud_region}` : null,
    verdict?.novelty_score !== undefined
      ? `novelty ${Number(verdict.novelty_score).toFixed(3)}`
      : null,
    verdict?.latency_ms !== undefined ? `${verdict.latency_ms}ms triage` : null,
  ].filter(Boolean);
  return parts.length > 0 ? parts.join(" · ") : alert.summary;
}
