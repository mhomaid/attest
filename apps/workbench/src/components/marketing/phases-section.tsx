"use client";

import { motion } from "framer-motion";
import {
  BrainCircuit,
  Cpu,
  Database,
  FileCode2,
  Fingerprint,
  GitPullRequest,
  Key,
  Layers,
  Lock,
  Network,
  Search,
  Server,
  ShieldAlert,
  Workflow,
} from "lucide-react";
import { useRef } from "react";

interface Feature {
  title: string;
  description: string;
  tags: string[];
  icon: React.ElementType;
  status: "shipped" | "partial" | "planned";
}

const features: Feature[] = [
  {
    title: "Streaming Substrate",
    description:
      "Rust collector ingests raw CloudTrail events, normalises to OCSF 1.3, and publishes to Kafka (Redpanda locally) at 100k+ events/sec. No JVM, no lock-in.",
    tags: ["Rust", "Kafka / Redpanda", "OCSF 1.3"],
    icon: Workflow,
    status: "shipped",
  },
  {
    title: "Warm Storage & Analytics",
    description:
      "Parquet on MinIO with daily Hive partitions. ClickHouse s3() reads the same files: sub-30s aggregates, no second copy of the lake.",
    tags: ["Apache Iceberg", "ClickHouse", "Parquet"],
    icon: Layers,
    status: "shipped",
  },
  {
    title: "Detection Runtime",
    description:
      "HELIQL is a portable DSL that compiles to RisingWave streaming SQL and ClickHouse batch. 10 reference rules shipped in detections/.",
    tags: ["HELIQL DSL", "RisingWave", "Sigma-compatible"],
    icon: FileCode2,
    status: "shipped",
  },
  {
    title: "ML Classifier",
    description:
      "XGBoost triage model served via tract-onnx, P99 under 5 ms per inference (CI-asserted). SHAP explanations and a calibration sidecar keep confidence scores honest.",
    tags: ["XGBoost", "ONNX", "<5 ms", "SHAP"],
    icon: BrainCircuit,
    status: "shipped",
  },
  {
    title: "LLM Investigator",
    description:
      "OpenAI-compat models escalate low-confidence cases, call MCP tools, write structured reasoning, and emit a second signed envelope.",
    tags: ["Qwen", "Anthropic", "MCP tools"],
    icon: Server,
    status: "shipped",
  },
  {
    title: "MCP Gateway",
    description:
      "Rust gateway intercepts every tool call before execution. Policy-engine RBAC by agent role, argument hashing, and a warm-tier rate limiter.",
    tags: ["Rust", "MCP", "RBAC"],
    icon: Lock,
    status: "shipped",
  },
  {
    title: "Hybrid Orchestrator",
    description:
      "Classifier + LLM paths with novelty routing, shadow checks, and calibrated auto-close. When scores diverge, the case gets a second look.",
    tags: ["Shadow checks", "Calibration", "Auto-close"],
    icon: Network,
    status: "shipped",
  },
  {
    title: "Ed25519 Attestation",
    description:
      "Every agent decision is a signed envelope: artifact hashes, feature vector, tool-call log, verdict. Tampering fails verification; inspect it in the workbench trace panel.",
    tags: ["Ed25519", "Signed envelopes", "Tamper-evident"],
    icon: Key,
    status: "shipped",
  },
  {
    title: "Analyst Workbench",
    description:
      "Dense Next.js SOC UI: live alert queue, hybrid case investigation, Simulate Lab, Load Lab, and admin health. Hunt and coverage are still thin.",
    tags: ["Next.js 16", "WebSocket", "Zustand"],
    icon: Cpu,
    status: "partial",
  },
  {
    title: "Threat Hunter Agent",
    description:
      "Natural-language hunt queries translated to HELIQL against warm Iceberg. The query editor is live; the agent that authors hunts is not.",
    tags: ["NL → HELIQL", "Iceberg hunt"],
    icon: Search,
    status: "planned",
  },
  {
    title: "Detection Engineer Agent",
    description:
      "Proposes HELIQL from coverage gaps, backtests, and opens a human-reviewed Git PR. Specified as SIDM; not in this MVP.",
    tags: ["SIDM", "Backtest", "PR review"],
    icon: GitPullRequest,
    status: "planned",
  },
  {
    title: "Responder Agent",
    description:
      "Policy-gated containment: isolate host, revoke session, ticket. Explicitly out of MVP; recommendations only until shadow-check is mature.",
    tags: ["Out of MVP", "SOAR later"],
    icon: ShieldAlert,
    status: "planned",
  },
  {
    title: "AI-Threat Detection",
    description:
      "Treat AI agents as first-class OCSF entities. OTEL GenAI + MCP traces, five launch detections (prompt injection, tool misuse, PII leak). Phase 9.",
    tags: ["AADF", "OTEL GenAI", "Phase 9"],
    icon: Fingerprint,
    status: "planned",
  },
  {
    title: "Enterprise & MSSP",
    description:
      "Multi-tenant isolation, BYOK, federated push-down, OIDC SSO, SOC 2 exports. Architecture supports it; this demo is a single-tenant Railway stack.",
    tags: ["GA v1", "BYOC", "OIDC"],
    icon: Database,
    status: "planned",
  },
];

export function PhasesSection() {
  const trackRef = useRef<HTMLDivElement>(null);

  return (
    <section id="phases" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6 mb-10">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Platform capabilities
        </p>
        <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
          Everything in the stack
        </h2>
        <p className="mt-4 max-w-3xl text-muted-foreground">
          From raw ingest to signed verdicts, each capability is a discrete
          layer. Cards tagged shipped are in this repo with an E2E gate;
          planned cards are the blueprint, not a claim.
        </p>
      </div>

      {/* Carousel track */}
      <div
        ref={trackRef}
        className="flex gap-4 overflow-x-auto px-4 sm:px-8 pb-4 scrollbar-none cursor-grab active:cursor-grabbing select-none"
        style={{ scrollSnapType: "x mandatory" }}
        onMouseDown={(e) => {
          const el = trackRef.current;
          if (!el) return;
          const startX = e.pageX - el.offsetLeft;
          const scrollLeft = el.scrollLeft;
          const onMove = (ev: MouseEvent) => {
            el.scrollLeft = scrollLeft - (ev.pageX - el.offsetLeft - startX);
          };
          const onUp = () => {
            window.removeEventListener("mousemove", onMove);
            window.removeEventListener("mouseup", onUp);
          };
          window.addEventListener("mousemove", onMove);
          window.addEventListener("mouseup", onUp);
        }}
      >
        {features.map((feat, i) => (
          <motion.div
            key={feat.title}
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true, margin: "-40px" }}
            transition={{ duration: 0.45, delay: 0.04 * (i % 4), ease: [0.22, 1, 0.36, 1] }}
            style={{ scrollSnapAlign: "start" }}
            className="group relative flex w-[calc(25vw-28px)] min-w-[240px] max-w-[340px] shrink-0 flex-col rounded-2xl border border-border/70 bg-card/50 p-6 backdrop-blur-sm transition-all duration-300 hover:border-primary/40 hover:bg-card/80 hover:shadow-[0_0_32px_-8px] hover:shadow-primary/15"
          >
            {/* Subtle glow on hover */}
            <div className="pointer-events-none absolute -right-8 -top-8 h-32 w-32 rounded-full bg-primary/10 opacity-0 blur-2xl transition-opacity duration-500 group-hover:opacity-100" />

            {/* Icon */}
            <div className="relative z-10 grid h-9 w-9 place-items-center rounded-lg border border-primary/25 bg-primary/10 transition-colors group-hover:border-primary/45 group-hover:bg-primary/15">
              <feat.icon className="h-4 w-4 text-primary" aria-hidden />
            </div>

            <div className="relative z-10 mt-4 flex items-start justify-between gap-2">
              <h3 className="text-sm font-semibold leading-snug">{feat.title}</h3>
              <span
                className={
                  feat.status === "shipped"
                    ? "shrink-0 font-mono text-[9px] uppercase tracking-wide text-signal-good"
                    : feat.status === "partial"
                      ? "shrink-0 font-mono text-[9px] uppercase tracking-wide text-foreground"
                      : "shrink-0 font-mono text-[9px] uppercase tracking-wide text-muted-foreground"
                }
              >
                {feat.status}
              </span>
            </div>

            {/* Description */}
            <p className="relative z-10 mt-2 flex-1 text-sm leading-relaxed text-muted-foreground">
              {feat.description}
            </p>

            {/* Tags */}
            <div className="relative z-10 mt-5 flex flex-wrap gap-1.5">
              {feat.tags.map((tag) => (
                <span
                  key={tag}
                  className="rounded-md border border-border/50 bg-secondary/50 px-2 py-0.5 font-mono text-[10px] text-muted-foreground"
                >
                  {tag}
                </span>
              ))}
            </div>
          </motion.div>
        ))}

        {/* End spacer */}
        <div className="w-4 shrink-0" />
      </div>
    </section>
  );
}
