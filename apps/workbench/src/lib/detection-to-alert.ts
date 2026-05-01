/**
 * Maps a fired detection row (from /v1/detections/fired) to the Alert shape
 * used by AlertQueue.
 */
import type { Alert, AgentStatus, Severity } from "@/lib/mock-data";

export type FiredDetection = {
  detection_id: string;
  event_id: string;
  actor_user_name: string;
  cloud_region: string;
  severity: string;
  fired_at: string;
};

const IDLE_AGENT: AgentStatus = {
  agent: "Triager",
  status: "idle",
  step: "Awaiting triage",
  toolsUsed: 0,
};

/** Human-readable titles per detection_id. */
const DETECTION_TITLES: Record<string, string> = {
  aws_console_login_from_anomalous_geolocation: "Console Login from Anomalous Region",
  aws_cloudtrail_logging_disabled:             "CloudTrail Logging Disabled",
  aws_root_account_use:                        "AWS Root Account Used",
  aws_iam_user_excessive_privilege:            "Excessive IAM Privilege Granted",
  aws_new_iam_user_then_access_keys_sequence:  "New IAM User Created with Access Keys",
  aws_s3_bucket_policy_made_public:            "S3 Bucket Made Public",
  okta_brute_force_authentication:             "Okta Brute-Force Authentication",
  okta_mfa_bypass_attempt:                     "Okta MFA Bypass Attempt",
  m365_mass_external_sharing:                  "M365 Mass External Sharing",
  m365_inbox_rule_auto_forward_external:       "M365 Inbox Auto-Forward Rule",
};

/** Primary MITRE technique per detection. */
const DETECTION_MITRE: Record<string, string> = {
  aws_console_login_from_anomalous_geolocation: "T1078.004",
  aws_cloudtrail_logging_disabled:             "T1562.001",
  aws_root_account_use:                        "T1078",
  aws_iam_user_excessive_privilege:            "T1098",
  aws_new_iam_user_then_access_keys_sequence:  "T1136.003",
  aws_s3_bucket_policy_made_public:            "T1530",
  okta_brute_force_authentication:             "T1110",
  okta_mfa_bypass_attempt:                     "T1556",
  m365_mass_external_sharing:                  "T1567",
  m365_inbox_rule_auto_forward_external:       "T1114.003",
};

function toTitleCase(id: string): string {
  return id
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

function mapSeverity(raw: string): Severity {
  const s = raw.toLowerCase();
  if (s === "critical" || s === "high" || s === "medium" || s === "low") {
    return s as Severity;
  }
  return "medium";
}

function relativeTime(iso: string): string {
  try {
    // RisingWave returns timestamps like "2026-04-27 05:36:45.785834+00:00"
    const date = new Date(iso.replace(" ", "T"));
    const diffMs = Date.now() - date.getTime();
    const diffS = Math.floor(diffMs / 1000);
    if (diffS < 60) return `${diffS}s ago`;
    const diffM = Math.floor(diffS / 60);
    if (diffM < 60) return `${diffM}m ago`;
    const diffH = Math.floor(diffM / 60);
    if (diffH < 24) return `${diffH}h ago`;
    return `${Math.floor(diffH / 24)}d ago`;
  } catch {
    return iso;
  }
}

export function firedDetectionToAlert(d: FiredDetection): Alert {
  const title =
    DETECTION_TITLES[d.detection_id] ?? toTitleCase(d.detection_id);
  const technique = DETECTION_MITRE[d.detection_id] ?? "";

  return {
    id:            d.event_id,
    caseId:        d.event_id,
    title,
    source:        "HELIQL Detection",
    entity:        d.actor_user_name || "unknown",
    region:        d.cloud_region    || "unknown",
    severity:      mapSeverity(d.severity),
    state:         "awaiting-review",
    verdict:       "investigating",
    confidence:    0,
    executionPath: "classifier",
    updatedAt:     d.fired_at,  // raw ISO — relative label computed live in UI
    technique,
    summary:       `Detection rule "${d.detection_id}" fired — entity: ${d.actor_user_name}, region: ${d.cloud_region}.`,
    agentStatus:   IDLE_AGENT,
  };
}

export function parseFiredDetections(data: unknown): Alert[] {
  if (!Array.isArray(data)) return [];
  return (data as FiredDetection[]).map(firedDetectionToAlert);
}
