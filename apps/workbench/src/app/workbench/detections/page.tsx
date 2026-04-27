import { FileCode2, ShieldAlert } from "lucide-react";
import { StatusBadge } from "@/components/workbench/status-badge";
import type { Severity } from "@/lib/mock-data";
import type { FiredDetection } from "@/lib/detection-to-alert";
import { cn } from "@/lib/utils";

// ── Bundled detection rule metadata ──────────────────────────────────────────

type RuleMeta = {
  id:          string;
  title:       string;
  description: string;
  severity:    Severity;
  mitre:       string[];
  applies_to:  string;
};

const RULES: RuleMeta[] = [
  {
    id:          "aws_console_login_from_anomalous_geolocation",
    title:       "Console Login from Anomalous Region",
    description: "Detects AWS console login from a region the user has not authenticated from in the past 90 days.",
    severity:    "medium",
    mitre:       ["T1078.004"],
    applies_to:  "ocsf.authentication",
  },
  {
    id:          "aws_cloudtrail_logging_disabled",
    title:       "CloudTrail Logging Disabled",
    description: "Detects StopLogging, DeleteTrail, or UpdateTrail against cloudtrail.amazonaws.com — a common defence-evasion step.",
    severity:    "critical",
    mitre:       ["T1562.001"],
    applies_to:  "ocsf.cloud_activity",
  },
  {
    id:          "aws_root_account_use",
    title:       "AWS Root Account Used",
    description: "Detects any use of the AWS root account, which should never occur in normal operations.",
    severity:    "critical",
    mitre:       ["T1078"],
    applies_to:  "ocsf.authentication",
  },
  {
    id:          "aws_iam_user_excessive_privilege",
    title:       "Excessive IAM Privilege Granted",
    description: "Detects AdministratorAccess or PowerUserAccess policies attached to an IAM entity.",
    severity:    "high",
    mitre:       ["T1098"],
    applies_to:  "ocsf.cloud_activity",
  },
  {
    id:          "aws_new_iam_user_then_access_keys_sequence",
    title:       "New IAM User + Access Keys Sequence",
    description: "Detects CreateUser followed by CreateAccessKey for the same user within a short window.",
    severity:    "high",
    mitre:       ["T1136.003"],
    applies_to:  "ocsf.cloud_activity",
  },
  {
    id:          "aws_s3_bucket_policy_made_public",
    title:       "S3 Bucket Made Public",
    description: "Detects PutBucketPolicy or PutBucketAcl operations that expose a bucket publicly.",
    severity:    "high",
    mitre:       ["T1530"],
    applies_to:  "ocsf.cloud_activity",
  },
  {
    id:          "okta_brute_force_authentication",
    title:       "Okta Brute-Force Authentication",
    description: "Detects a high volume of failed Okta logins (unique window) suggestive of credential stuffing.",
    severity:    "high",
    mitre:       ["T1110"],
    applies_to:  "ocsf.authentication",
  },
  {
    id:          "okta_mfa_bypass_attempt",
    title:       "Okta MFA Bypass Attempt",
    description: "Detects MFA factor rejection or reset events from an unknown device.",
    severity:    "high",
    mitre:       ["T1556"],
    applies_to:  "ocsf.authentication",
  },
  {
    id:          "m365_mass_external_sharing",
    title:       "M365 Mass External Sharing",
    description: "Detects a burst of SharePoint/OneDrive sharing events to external domains.",
    severity:    "high",
    mitre:       ["T1567"],
    applies_to:  "ocsf.cloud_activity",
  },
  {
    id:          "m365_inbox_rule_auto_forward_external",
    title:       "M365 Inbox Auto-Forward Rule",
    description: "Detects New-InboxRule events that forward mail to an external address.",
    severity:    "critical",
    mitre:       ["T1114.003"],
    applies_to:  "ocsf.cloud_activity",
  },
];

// ── Severity helpers ──────────────────────────────────────────────────────────

const severityOrder: Record<Severity, number> = {
  critical: 0, high: 1, medium: 2, low: 3,
};

// ── Data fetching ─────────────────────────────────────────────────────────────

const API_URL = process.env.NEXT_PUBLIC_APP_URL
  ? `${process.env.NEXT_PUBLIC_APP_URL}/api/detections`
  : "http://localhost:3000/api/detections";

async function fetchLastFired(): Promise<Map<string, string>> {
  try {
    const res = await fetch(API_URL, { cache: "no-store" });
    if (!res.ok) return new Map();
    const data = (await res.json()) as FiredDetection[];
    const map = new Map<string, string>();
    for (const d of data) {
      // Keep only the most-recent fired_at per detection_id.
      if (!map.has(d.detection_id) || d.fired_at > map.get(d.detection_id)!) {
        map.set(d.detection_id, d.fired_at);
      }
    }
    return map;
  } catch {
    return new Map();
  }
}

function relativeTime(iso: string): string {
  try {
    const date = new Date(iso.replace(" ", "T"));
    const diffS = Math.floor((Date.now() - date.getTime()) / 1000);
    if (diffS < 60) return `${diffS}s ago`;
    const diffM = Math.floor(diffS / 60);
    if (diffM < 60) return `${diffM}m ago`;
    const diffH = Math.floor(diffM / 60);
    if (diffH < 24) return `${diffH}h ago`;
    return `${Math.floor(diffH / 24)}d ago`;
  } catch {
    return "—";
  }
}

// ── Page ──────────────────────────────────────────────────────────────────────

export default async function DetectionsPage() {
  const lastFired = await fetchLastFired();

  const sorted = [...RULES].sort(
    (a, b) => severityOrder[a.severity] - severityOrder[b.severity],
  );

  const firedCount = sorted.filter((r) => lastFired.has(r.id)).length;

  return (
    <div className="space-y-3 p-3">
      {/* Header */}
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="mb-2 flex items-center gap-2">
              <StatusBadge tone="info">Detections</StatusBadge>
              <StatusBadge tone={firedCount > 0 ? "high" : "muted"}>
                {firedCount} rule{firedCount !== 1 ? "s" : ""} fired
              </StatusBadge>
            </div>
            <h1 className="text-xl font-semibold tracking-tight">Detection Rules</h1>
            <p className="mt-1 max-w-3xl text-sm text-muted-foreground">
              {sorted.length} bundled HELIQL rules compiled to RisingWave streaming SQL.
              Rules are checked against every event on the{" "}
              <code className="rounded bg-secondary px-1 py-0.5 font-mono text-[11px]">
                cloudtrail
              </code>{" "}
              topic in real time.
            </p>
          </div>

          <div className="grid grid-cols-3 gap-2">
            {(["critical", "high", "medium"] as Severity[]).map((sev) => (
              <div
                key={sev}
                className="rounded-md border border-border bg-background/60 px-3 py-2"
              >
                <div className="font-mono text-lg font-semibold">
                  {sorted.filter((r) => r.severity === sev).length}
                </div>
                <div className="mt-1 text-[11px] capitalize text-muted-foreground">{sev}</div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Rules table */}
      <section className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-card/95 text-left text-[11px] uppercase tracking-wider text-muted-foreground">
              <th className="px-3 py-2 font-medium">Rule</th>
              <th className="px-3 py-2 font-medium">Severity</th>
              <th className="hidden px-3 py-2 font-medium lg:table-cell">MITRE</th>
              <th className="hidden px-3 py-2 font-medium xl:table-cell">Applies to</th>
              <th className="px-3 py-2 font-medium">Last fired</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border/80">
            {sorted.map((rule) => {
              const fired = lastFired.get(rule.id);
              return (
                <tr
                  key={rule.id}
                  className={cn(
                    "transition-colors hover:bg-secondary/50",
                    fired && "bg-signal-live/5",
                  )}
                >
                  {/* Rule name + description */}
                  <td className="px-3 py-3">
                    <div className="flex items-center gap-2">
                      <FileCode2 className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                      <div className="min-w-0">
                        <div className="font-medium leading-snug">{rule.title}</div>
                        <div className="mt-0.5 line-clamp-1 max-w-xs text-xs text-muted-foreground">
                          {rule.description}
                        </div>
                        <code className="mt-0.5 block font-mono text-[10px] text-muted-foreground/60">
                          {rule.id}
                        </code>
                      </div>
                    </div>
                  </td>

                  {/* Severity */}
                  <td className="px-3 py-3">
                    <StatusBadge
                      tone={rule.severity as "critical" | "high" | "medium" | "low"}
                    >
                      {rule.severity}
                    </StatusBadge>
                  </td>

                  {/* MITRE */}
                  <td className="hidden px-3 py-3 lg:table-cell">
                    <div className="flex flex-wrap gap-1">
                      {rule.mitre.map((t) => (
                        <span
                          key={t}
                          className="rounded border border-border/60 bg-secondary/60 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
                        >
                          {t}
                        </span>
                      ))}
                    </div>
                  </td>

                  {/* Applies to */}
                  <td className="hidden px-3 py-3 xl:table-cell">
                    <span className="rounded border border-border/50 bg-secondary/40 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                      {rule.applies_to}
                    </span>
                  </td>

                  {/* Last fired */}
                  <td className="px-3 py-3">
                    {fired ? (
                      <span className="inline-flex items-center gap-1 text-xs text-foreground">
                        <ShieldAlert className="h-3.5 w-3.5 text-signal-good" />
                        {relativeTime(fired)}
                      </span>
                    ) : (
                      <span className="font-mono text-[11px] text-muted-foreground/60">
                        never
                      </span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </section>
    </div>
  );
}
