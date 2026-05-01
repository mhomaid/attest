import { Settings } from "lucide-react";
import { StatusBadge } from "@/components/workbench/status-badge";

const sections = [
  {
    title: "Integrations",
    items: [
      { label: "AWS CloudTrail",    status: "connected",    detail: "Streaming via Kafka `cloudtrail` topic" },
      { label: "Okta System Log",   status: "not-configured", detail: "Configure Okta API token to enable" },
      { label: "Microsoft 365",     status: "not-configured", detail: "Configure M365 connector" },
      { label: "Arroyo Pipelines",  status: "connected",    detail: "cloudtrail_to_parquet + cep_sequence_detection running" },
      { label: "MinIO Warm Tier",   status: "connected",    detail: "attest-warm bucket · Parquet format" },
      { label: "ClickHouse",        status: "connected",    detail: "Columnar query engine for warm tier" },
    ],
  },
  {
    title: "Detection Rules",
    items: [
      { label: "HELIQL rule sync",     status: "connected",      detail: "10 rules loaded from /rules/detections/" },
      { label: "Rule hot-reload",      status: "connected",      detail: "detection-runtime polls every 5s" },
      { label: "Arroyo CEP rules",     status: "connected",      detail: "cep_sequence_detection deployed" },
      { label: "Custom rule upload",   status: "not-configured", detail: "Coming in Phase 5 — Detection Engineer agent" },
    ],
  },
  {
    title: "AI Agents",
    items: [
      { label: "Triager (XGBoost)",    status: "connected",      detail: "Online · classifier path < 50ms P99" },
      { label: "Triager (LLM escalation)", status: "connected",  detail: "Claude Sonnet · hybrid path" },
      { label: "Investigator agent",   status: "connected",      detail: "Claude Opus / Qwen 3 32B (air-gapped)" },
      { label: "Hunter agent",         status: "connected",      detail: "Claude Sonnet" },
      { label: "Detection Eng. agent", status: "not-configured", detail: "Phase 10 — not yet deployed" },
    ],
  },
  {
    title: "Notifications",
    items: [
      { label: "Slack webhook",    status: "not-configured", detail: "No webhook URL configured" },
      { label: "PagerDuty",        status: "not-configured", detail: "No routing key configured" },
      { label: "Email digest",     status: "not-configured", detail: "No SMTP configured" },
    ],
  },
];

const tonemap: Record<string, "good" | "muted" | "info"> = {
  "connected":       "good",
  "not-configured":  "muted",
};

export default function SettingsPage() {
  return (
    <div className="space-y-3 p-3">
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="mb-2 flex items-center gap-2">
          <StatusBadge tone="info">Settings</StatusBadge>
        </div>
        <h1 className="text-xl font-semibold tracking-tight">Platform Settings</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Integration status, agent configuration, and notification routing.
        </p>
      </section>

      {sections.map((section) => (
        <section
          key={section.title}
          className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm"
        >
          <header className="flex items-center gap-2 border-b border-border bg-card/95 px-3 py-2">
            <Settings className="h-4 w-4 text-muted-foreground" />
            <h2 className="text-sm font-semibold">{section.title}</h2>
          </header>
          <div className="divide-y divide-border/80">
            {section.items.map((item) => (
              <div
                key={item.label}
                className="flex items-center justify-between gap-3 px-3 py-2.5 transition-colors hover:bg-secondary/40"
              >
                <div className="min-w-0">
                  <div className="text-sm font-medium">{item.label}</div>
                  <div className="mt-0.5 text-xs text-muted-foreground">{item.detail}</div>
                </div>
                <StatusBadge tone={tonemap[item.status] ?? "muted"}>
                  {item.status.replace("-", " ")}
                </StatusBadge>
              </div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
