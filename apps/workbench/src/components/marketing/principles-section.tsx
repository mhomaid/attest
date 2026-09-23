const principles = [
  {
    id: "P1",
    title: "Agents act through governed tools",
    body: "No raw cloud keys, no direct SQL, no unaudited response. MCP gateway authenticates the tool, the policy engine authorizes it, the attestation layer signs the call.",
  },
  {
    id: "P2",
    title: "Every decision is on the record",
    body: "Model id, prompt hash, tool-call hashes, intermediate beliefs, evidence citations, calibrated confidence. Signed together, so any edit is detectable. Deterministic replay of classifier decisions is next.",
  },
  {
    id: "P3",
    title: "Confidence is calibrated, not self-reported",
    body: "Isotonic regression maps raw scores to probability of correctness against ground truth. Brier and ECE are first-class metrics, not dashboard decoration.",
  },
  {
    id: "P4",
    title: "High-impact actions get a shadow check",
    body: "Do-not-touch lists, blast-radius caps, deny-by-default, rate limits. If the deterministic check disagrees with the agent, the case pauses for a human.",
  },
  {
    id: "P5",
    title: "Hallucinations are caught",
    body: "Retrieval before reasoning. Every claim cites an OCSF event id. Orphan citations become needs_investigation. High-impact verdicts can take a second specialist.",
  },
  {
    id: "P6",
    title: "Specialization beats one mega-agent",
    body: "Triager, Investigator, Hunter, Detection Engineer, Responder: distinct prompts, models, tool catalogs, and policies. The coordinator plans; it does not verdict.",
  },
  {
    id: "P7",
    title: "Humans set policy. Agents cannot",
    body: "The policy engine is outside the LLM stack and changes only through reviewed Git. An agent may request a change; it cannot enact one.",
  },
  {
    id: "P8",
    title: "Streaming-first, customer-owned data",
    body: "OCSF + Iceberg on the customer’s bucket. Leave in 24 hours and query the same Parquet with DuckDB, Snowflake, or ClickHouse. Lock-in is not the moat.",
  },
] as const;

export function PrinciplesSection() {
  return (
    <section id="design" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Design
        </p>
        <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
          Principles that are load-bearing
        </h2>
        <p className="mt-4 max-w-3xl text-muted-foreground">
          Attest is not “an LLM with tools pointed at alerts.” The difference
          between a verifiable agentic SIEM and AI security theater is these
          eight constraints. They are implemented in Rust, not in a prompt.
        </p>

        <ul className="mt-10 grid gap-3 md:grid-cols-2">
          {principles.map((p) => (
            <li
              key={p.id}
              className="rounded-xl border border-border/70 bg-card/40 p-5"
            >
              <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-primary">
                {p.id}
              </p>
              <h3 className="mt-2 text-sm font-semibold">{p.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                {p.body}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
