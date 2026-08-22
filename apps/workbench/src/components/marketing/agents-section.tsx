import { BranchFlow } from "@/components/marketing/flow-diagram";

const agents = [
  {
    name: "Coordinator",
    status: "live" as const,
    model: "Deterministic router in the orchestrator",
    body: "Receives an alert, chooses classifier vs LLM vs Investigator. Emits a plan, not a verdict. Cannot touch customer infrastructure.",
  },
  {
    name: "Hybrid Triager",
    status: "live" as const,
    model: "XGBoost ONNX · ~28 ms P99 · LLM escalation",
    body: "Eight-feature classifier, Mahalanobis novelty, isotonic calibration. In-distribution high-confidence cases stay on the classifier path. Novel or low-confidence cases escalate.",
  },
  {
    name: "Investigator",
    status: "live" as const,
    model: "OpenAI-compat · Qwen / Anthropic",
    body: "Tool loop through the MCP gateway (hot tier, warm SQL, threat-intel stubs). Second signed envelope. Workbench renders the replayable trace.",
  },
  {
    name: "Hunter",
    status: "shell" as const,
    model: "Phase 8+",
    body: "Hypothesis-driven hunts over Iceberg. The hunt page already runs warm SQL; the agent that authors those queries is not wired yet.",
  },
  {
    name: "Detection Engineer",
    status: "planned" as const,
    model: "Phase 10 · SIDM",
    body: "Coverage scan → HELIQL draft → backtest → Git PR. Humans merge. Shadow deploy, then promote. Not in this MVP.",
  },
  {
    name: "Responder",
    status: "out" as const,
    model: "Explicitly out of MVP",
    body: "Containment recommendations only. Destructive autonomy waits until shadow-check policy is mature enough for a regulated CISO.",
  },
] as const;

const statusLabel = {
  live: "Live in this build",
  shell: "UI shell",
  planned: "Roadmap",
  out: "Out of MVP",
} as const;

const statusClass = {
  live: "border-signal-good/40 bg-signal-good/10 text-signal-good",
  shell: "border-border bg-secondary text-muted-foreground",
  planned: "border-border bg-secondary text-muted-foreground",
  out: "border-border/50 bg-transparent text-muted-foreground",
} as const;

export function AgentsSection() {
  return (
    <section id="agents" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Agentic plane
        </p>
        <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
          Five specialists, one coordinator
        </h2>
        <p className="mt-4 max-w-3xl text-muted-foreground">
          Status labels are honest. The Triager and Investigator are real
          services with E2E tests. Hunter, Detection Engineer, and Responder
          are specified in the blueprint and shown here so the design is
          complete — they are not claimed as shipped.
        </p>

        <BranchFlow
          className="mt-10"
          start={{
            label: "Coordinator",
            hint: "Routes the alert",
            detail: "Receives an alert and chooses classifier vs Investigator. Emits a plan, not a verdict.",
          }}
          left={{
            label: "Classifier",
            hint: "ONNX · ~28 ms",
            detail: "In-distribution high-confidence cases stay on XGBoost. SHAP + calibration, no LLM.",
          }}
          right={{
            label: "Investigator",
            hint: "LLM · MCP tools",
            detail: "Novel or low-confidence cases escalate. Tools only move through the gateway.",
          }}
          join={{
            label: "Attest",
            hint: "Ed25519 envelope",
            detail: "Both paths emit a signed envelope: hashes, tools, verdict, replayable later.",
          }}
          end={{
            label: "Workbench",
            hint: "Queue + trace",
            detail: "The analyst sees the same artifacts — SHAP bars or the investigator stepper.",
          }}
        />

        <ul className="mt-10 grid gap-3 md:grid-cols-2">
          {agents.map((agent) => (
            <li
              key={agent.name}
              className="flex flex-col rounded-xl border border-border/70 bg-card/40 p-5"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h3 className="text-sm font-semibold">{agent.name}</h3>
                <span
                  className={`rounded-md border px-2 py-0.5 font-mono text-[10px] uppercase tracking-wide ${statusClass[agent.status]}`}
                >
                  {statusLabel[agent.status]}
                </span>
              </div>
              <p className="mt-2 font-mono text-[11px] text-muted-foreground">
                {agent.model}
              </p>
              <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                {agent.body}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
