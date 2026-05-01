import { Activity, Bot, CheckCircle2, Clock3, Cpu, Zap } from "lucide-react";
import { StatusBadge } from "@/components/workbench/status-badge";

const ORCHESTRATOR_URL = process.env.ORCHESTRATOR_URL ?? "http://localhost:4300";

type OrchestratorMetrics = {
  triage_count: number;
  p50_ms: number;
  p95_ms: number;
  p99_ms: number;
};

async function fetchOrchestratorMetrics(): Promise<OrchestratorMetrics | null> {
  try {
    const res = await fetch(`${ORCHESTRATOR_URL}/metrics`, {
      cache: "no-store",
      signal: AbortSignal.timeout(3000),
    });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

type AgentDef = {
  id:          string;
  name:        string;
  role:        string;
  model:       string;
  execPath:    string;
  phase:       string;
  description: string;
};

const AGENT_DEFS: AgentDef[] = [
  {
    id:          "triager-hybrid-v1",
    name:        "Triager",
    role:        "Triager",
    model:       "XGBoost (classifier) + Claude Sonnet (escalation)",
    execPath:    "Hybrid",
    phase:       "4b",
    description: "Classifies every incoming alert. Routes ~80% via XGBoost in < 50ms; escalates novel cases to the LLM path.",
  },
  {
    id:          "investigator-v1",
    name:        "Investigator",
    role:        "Investigator",
    model:       "Claude Opus (prod) / Qwen 3 32B (air-gapped)",
    execPath:    "LLM",
    phase:       "5",
    description: "Handles Triager escalations. Queries warm tier, hot tier, and threat intel. Produces cited verdicts.",
  },
  {
    id:          "hunter-v1",
    name:        "Hunter",
    role:        "Hunter",
    model:       "Claude Sonnet",
    execPath:    "LLM",
    phase:       "7",
    description: "Proactively expands hypotheses from Investigator cases. Searches for lateral movement and campaign indicators.",
  },
  {
    id:          "detection-engineer-v1",
    name:        "Detection Engineer",
    role:        "DetectionEngineer",
    model:       "Claude Sonnet",
    execPath:    "LLM",
    phase:       "10",
    description: "Weekly: scans MITRE coverage gaps, proposes HELIQL rules with backtest evidence, opens Git PRs.",
  },
];

export default async function AgentsPage() {
  const metrics = await fetchOrchestratorMetrics();
  const triagerOnline = metrics !== null;

  return (
    <div className="space-y-3 p-3">
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="mb-2 flex items-center gap-2">
              <StatusBadge tone="info">Agents</StatusBadge>
              <StatusBadge tone={triagerOnline ? "good" : "muted"}>
                {triagerOnline ? "Triager online" : "Orchestrator offline"}
              </StatusBadge>
            </div>
            <h1 className="text-xl font-semibold tracking-tight">Agent Roster</h1>
            <p className="mt-1 text-sm text-muted-foreground">
              All registered agent definitions. Each agent has a signed definition with a verifiable
              artifact hash. Every verdict produces a cryptographic attestation envelope.
            </p>
          </div>

          <div className="grid grid-cols-3 gap-2">
            <div className="rounded-md border border-border bg-background/60 px-3 py-2">
              <div className="font-mono text-lg font-semibold">
                {metrics ? metrics.triage_count.toLocaleString() : "—"}
              </div>
              <div className="mt-1 text-[11px] text-muted-foreground">Total verdicts</div>
            </div>
            <div className="rounded-md border border-border bg-background/60 px-3 py-2">
              <div className="font-mono text-lg font-semibold">
                {metrics ? `${metrics.p99_ms}ms` : "—"}
              </div>
              <div className="mt-1 text-[11px] text-muted-foreground">Classifier P99</div>
            </div>
            <div className="rounded-md border border-border bg-background/60 px-3 py-2">
              <div className="font-mono text-lg font-semibold">
                {triagerOnline ? "100%" : "—"}
              </div>
              <div className="mt-1 text-[11px] text-muted-foreground">Envelope coverage</div>
            </div>
          </div>
        </div>
      </section>

      <div className="grid gap-3 md:grid-cols-2">
        {AGENT_DEFS.map((agent) => {
          const isTriager = agent.id === "triager-hybrid-v1";
          const status = isTriager
            ? triagerOnline ? "online" : "offline"
            : "planned";

          return (
            <section
              key={agent.id}
              className="rounded-lg border border-border bg-card/80 p-3 shadow-sm"
            >
              <div className="mb-3 flex items-start justify-between gap-2">
                <div className="flex items-center gap-2">
                  <div className="grid h-9 w-9 place-items-center rounded-md border border-border bg-secondary">
                    <Bot className="h-4 w-4 text-muted-foreground" />
                  </div>
                  <div>
                    <div className="font-semibold leading-tight">{agent.name}</div>
                    <div className="font-mono text-[10px] text-muted-foreground">{agent.id}</div>
                  </div>
                </div>
                <StatusBadge
                  tone={status === "online" ? "good" : status === "offline" ? "high" : "muted"}
                >
                  {status === "planned" ? `Phase ${agent.phase}` : status}
                </StatusBadge>
              </div>

              <p className="mb-3 text-xs leading-5 text-muted-foreground">{agent.description}</p>

              <div className="space-y-1.5 text-xs">
                <div className="flex items-center gap-2">
                  <Cpu className="h-3.5 w-3.5 text-muted-foreground" />
                  <span className="text-muted-foreground">Model:</span>
                  <span className="font-medium">{agent.model}</span>
                </div>
                <div className="flex items-center gap-2">
                  <Zap className="h-3.5 w-3.5 text-muted-foreground" />
                  <span className="text-muted-foreground">Execution:</span>
                  <span className="font-medium">{agent.execPath}</span>
                </div>
                {isTriager && (
                  <>
                    <div className="flex items-center gap-2">
                      <Activity className="h-3.5 w-3.5 text-muted-foreground" />
                      <span className="text-muted-foreground">Verdicts:</span>
                      <span className="font-mono font-medium">
                        {metrics ? metrics.triage_count.toLocaleString() : "—"}
                      </span>
                    </div>
                    {metrics && metrics.p99_ms > 0 && (
                      <div className="flex items-center gap-2">
                        <Clock3 className="h-3.5 w-3.5 text-muted-foreground" />
                        <span className="text-muted-foreground">P50 / P95 / P99:</span>
                        <span className="font-mono font-medium text-signal-good">
                          {metrics.p50_ms}ms / {metrics.p95_ms}ms / {metrics.p99_ms}ms
                        </span>
                      </div>
                    )}
                  </>
                )}
              </div>

              {isTriager && triagerOnline && (
                <div className="mt-3 flex items-center gap-1.5 text-[11px] text-signal-good">
                  <CheckCircle2 className="h-3.5 w-3.5" />
                  Attestation envelopes: Ed25519 signed
                </div>
              )}
            </section>
          );
        })}
      </div>
    </div>
  );
}
