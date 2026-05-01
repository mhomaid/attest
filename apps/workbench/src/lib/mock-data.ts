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
  /** ISO-8601 timestamp — rendered as a live relative time in the UI. */
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

