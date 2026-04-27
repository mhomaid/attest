export type Severity = "critical" | "high" | "medium" | "low";
export type AlertState = "in-flight" | "awaiting-review" | "auto-closed";
export type ExecutionPath = "classifier" | "llm" | "hybrid";

export type FeatureImpact = {
  name: string;
  value: string;
  impact: number;
};

export type Alert = {
  id: string;
  caseId: string;
  title: string;
  source: string;
  entity: string;
  region: string;
  severity: Severity;
  state: AlertState;
  verdict: "benign" | "suspicious" | "malicious" | "investigating";
  confidence: number;
  executionPath: ExecutionPath;
  updatedAt: string;
  technique: string;
  summary: string;
  agentStatus: AgentStatus;
};

export type AgentStatus = {
  agent: "Triager" | "Investigator" | "Hunter";
  status: "idle" | "running" | "complete" | "degraded";
  step: string;
  toolsUsed: number;
};

export type CaseRecord = {
  id: string;
  title: string;
  severity: Severity;
  state: AlertState;
  entity: string;
  source: string;
  verdict: Alert["verdict"];
  confidence: number;
  executionPath: ExecutionPath;
  timeline: Array<{
    time: string;
    actor: string;
    event: string;
  }>;
  evidence: Array<{
    label: string;
    value: string;
    type: "ocsf" | "asset" | "threat-intel";
  }>;
  featureImpacts: FeatureImpact[];
};

export const alerts: Alert[] = [
  {
    id: "alert-9001",
    caseId: "case-1042",
    title: "Console login from anomalous geography",
    source: "AWS CloudTrail",
    entity: "alice@example.com",
    region: "ap-southeast-1",
    severity: "high",
    state: "awaiting-review",
    verdict: "suspicious",
    confidence: 0.87,
    executionPath: "classifier",
    updatedAt: "24s ago",
    technique: "T1078.004",
    summary:
      "First observed login from Singapore after a 30-day US-only baseline. MFA was present but device fingerprint is new.",
    agentStatus: {
      agent: "Triager",
      status: "complete",
      step: "Classifier verdict emitted",
      toolsUsed: 3,
    },
  },
  {
    id: "alert-9002",
    caseId: "case-1043",
    title: "Root account used to modify S3 bucket policy",
    source: "AWS CloudTrail",
    entity: "root@acme-prod",
    region: "us-east-1",
    severity: "critical",
    state: "in-flight",
    verdict: "investigating",
    confidence: 0.62,
    executionPath: "hybrid",
    updatedAt: "51s ago",
    technique: "T1078",
    summary:
      "Root principal changed public access block settings on a customer-data bucket. Investigator is collecting related object access.",
    agentStatus: {
      agent: "Investigator",
      status: "running",
      step: "Querying warm-tier S3 object access",
      toolsUsed: 5,
    },
  },
  {
    id: "alert-9003",
    caseId: "case-1044",
    title: "Okta brute-force followed by successful MFA",
    source: "Okta System Log",
    entity: "marco@example.com",
    region: "us-west-2",
    severity: "medium",
    state: "awaiting-review",
    verdict: "suspicious",
    confidence: 0.73,
    executionPath: "classifier",
    updatedAt: "2m ago",
    technique: "T1110",
    summary:
      "17 failed authentications from one ASN preceded a successful MFA-backed login on a new device.",
    agentStatus: {
      agent: "Triager",
      status: "complete",
      step: "Awaiting analyst review",
      toolsUsed: 4,
    },
  },
  {
    id: "alert-9004",
    caseId: "case-1045",
    title: "M365 inbox auto-forward rule to external domain",
    source: "Microsoft 365",
    entity: "finance.ops@example.com",
    region: "global",
    severity: "high",
    state: "in-flight",
    verdict: "investigating",
    confidence: 0.58,
    executionPath: "hybrid",
    updatedAt: "4m ago",
    technique: "T1564.008",
    summary:
      "Mailbox rule forwards invoices to a newly registered external domain. Hunter is searching adjacent users.",
    agentStatus: {
      agent: "Hunter",
      status: "running",
      step: "Expanding campaign hypothesis",
      toolsUsed: 7,
    },
  },
  {
    id: "alert-9005",
    caseId: "case-1046",
    title: "Low-risk login matches known travel pattern",
    source: "AWS CloudTrail",
    entity: "devrel@example.com",
    region: "eu-west-1",
    severity: "low",
    state: "auto-closed",
    verdict: "benign",
    confidence: 0.94,
    executionPath: "classifier",
    updatedAt: "9m ago",
    technique: "T1078.004",
    summary:
      "Classifier matched the travel calendar, prior laptop fingerprint, and known VPN egress pattern.",
    agentStatus: {
      agent: "Triager",
      status: "complete",
      step: "Auto-closed with attestation",
      toolsUsed: 3,
    },
  },
];

export const cases: CaseRecord[] = [
  {
    id: "case-1042",
    title: "Console login from anomalous geography",
    severity: "high",
    state: "awaiting-review",
    entity: "alice@example.com",
    source: "AWS CloudTrail",
    verdict: "suspicious",
    confidence: 0.87,
    executionPath: "classifier",
    timeline: [
      {
        time: "21:18:04",
        actor: "Collector",
        event: "Normalized CloudTrail ConsoleLogin to OCSF authentication event.",
      },
      {
        time: "21:18:04",
        actor: "Triager",
        event: "Fetched 30-day user baseline and recent asset context.",
      },
      {
        time: "21:18:04",
        actor: "Triager",
        event: "Classifier path emitted suspicious verdict in 38ms.",
      },
      {
        time: "21:18:05",
        actor: "Attestation",
        event: "Signed classifier envelope with SHAP feature attribution.",
      },
    ],
    evidence: [
      { label: "Event ID", value: "evt-aws-7f2c91", type: "ocsf" },
      { label: "User Baseline", value: "US-only login regions for 30 days", type: "asset" },
      { label: "Threat Intel", value: "ASN has 4 prior credential-stuffing hits", type: "threat-intel" },
      { label: "Device", value: "New browser fingerprint; no prior cookie", type: "asset" },
    ],
    featureImpacts: [
      { name: "New country", value: "Singapore", impact: 0.34 },
      { name: "Device novelty", value: "New fingerprint", impact: 0.22 },
      { name: "ASN reputation", value: "Elevated", impact: 0.16 },
      { name: "MFA present", value: "Yes", impact: -0.08 },
      { name: "Prior similar travel", value: "None", impact: 0.13 },
    ],
  },
];

export function getCaseById(id: string) {
  return cases.find((caseRecord) => caseRecord.id === id) ?? cases[0];
}
