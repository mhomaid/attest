import type { Alert, AgentStatus } from "./mock-data";

/** Shape returned by GET /v1/events/recent (array variant) */
export interface OcsfEvent {
  event_id: string;
  class_uid?: string;
  time?: string;
  tenant_id?: string;
  actor_user_name?: string;
  actor_user_uid?: string;
  cloud_region?: string;
  cloud_account_uid?: string;
  severity?: string;
  auth_status?: string;
  api_operation?: string;
  api_service?: string;
}

const IDLE_AGENT: AgentStatus = {
  agent: "Triager",
  status: "idle",
  step: "Awaiting triage (Phase 4)",
  toolsUsed: 0,
};

function mapSeverity(s?: string): Alert["severity"] {
  switch (s?.toLowerCase()) {
    case "critical":
      return "critical";
    case "high":
      return "high";
    case "medium":
      return "medium";
    default:
      return "low";
  }
}

function titleFromEvent(ev: OcsfEvent): string {
  if (ev.class_uid === "3002" || ev.class_uid === "Authentication") {
    const status = ev.auth_status === "Success" ? "successful" : "failed";
    return `${status} console login${ev.cloud_region ? ` (${ev.cloud_region})` : ""}`;
  }
  if (ev.api_operation) {
    return `${ev.api_operation}${ev.api_service ? ` via ${ev.api_service}` : ""}`;
  }
  return `OCSF event ${ev.event_id.slice(0, 8)}`;
}

export function ocsfEventToAlert(ev: OcsfEvent, index: number): Alert {
  return {
    id: ev.event_id,
    caseId: `case-${ev.event_id.slice(0, 8)}`,
    title: titleFromEvent(ev),
    source: "AWS CloudTrail",
    entity: ev.actor_user_name ?? ev.actor_user_uid ?? "unknown",
    region: ev.cloud_region ?? "—",
    severity: mapSeverity(ev.severity),
    state: "awaiting-review",
    verdict: "investigating",
    confidence: 0,
    executionPath: "classifier",
    updatedAt: ev.time ? new Date(ev.time).toLocaleTimeString() : `event ${index + 1}`,
    technique: ev.class_uid === "3002" ? "T1078.004" : "T1530",
    summary: `Live OCSF event from attest-collector. Triage agents available in Phase 4.`,
    agentStatus: IDLE_AGENT,
  };
}

export function parseEventsResponse(data: unknown): Alert[] {
  // The control-plane returns either a single event or an array.
  if (Array.isArray(data)) {
    return data.map((ev, i) => ocsfEventToAlert(ev as OcsfEvent, i));
  }
  if (data && typeof data === "object" && "event_id" in data) {
    return [ocsfEventToAlert(data as OcsfEvent, 0)];
  }
  return [];
}
