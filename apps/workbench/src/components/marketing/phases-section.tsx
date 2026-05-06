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
}

const features: Feature[] = [
  {
    title: "Streaming Substrate",
    description:
      "Rust collector ingests raw CloudTrail events, normalises to OCSF 1.3, and publishes to Redpanda at 100k+ events/sec. No JVM, no lock-in.",
    tags: ["Rust", "Redpanda", "OCSF 1.3"],
    icon: Workflow,
  },
  {
    title: "Warm Storage & Analytics",
    description:
      "Apache Iceberg on MinIO with daily tenant partitioning. ClickHouse delivers sub-30s aggregate queries over months of compressed history.",
    tags: ["Apache Iceberg", "ClickHouse", "Parquet"],
    icon: Layers,
  },
  {
    title: "Detection Runtime",
    description:
      "HELIQL — a portable DSL that compiles to RisingWave streaming SQL and ClickHouse batch. 10 reference rules shipped out of the box.",
    tags: ["HELIQL DSL", "RisingWave", "Sigma-compatible"],
    icon: FileCode2,
  },
  {
    title: "ML Classifier",
    description:
      "XGBoost triage model served via ONNX at <50 ms. SHAP explanations and a calibration sidecar keep confidence scores honest.",
    tags: ["XGBoost", "ONNX", "<50 ms", "SHAP"],
    icon: BrainCircuit,
  },
  {
    title: "LLM Investigator",
    description:
      "Open-weights models escalate low-confidence cases, call MCP tools, write structured reasoning, and emit a signed verdict — fully air-gapped if needed.",
    tags: ["Qwen 3.6", "Gemma", "GLM 5.1", "Mistral"],
    icon: Server,
  },
  {
    title: "MCP Gateway",
    description:
      "Rust gateway intercepts every tool call before execution. Policy-engine RBAC by agent role, signed attestations, and a hard kill-switch per tenant.",
    tags: ["Rust", "MCP / A2A", "RBAC"],
    icon: Lock,
  },
  {
    title: "Hybrid Orchestrator",
    description:
      "Combines the classifier and LLM paths with shadow checks — when calibrated scores diverge from the LLM verdict, the case gets a second look.",
    tags: ["Shadow checks", "Calibration gate", "Auto-close"],
    icon: Network,
  },
  {
    title: "Ed25519 Attestation",
    description:
      "Every agent decision is wrapped in a signed envelope: model artifact hash, feature vector, tool call log, and verdict — cryptographically reproducible.",
    tags: ["Ed25519", "Signed envelopes", "Replay"],
    icon: Key,
  },
  {
    title: "Analyst Workbench",
    description:
      "Bloomberg-density Next.js SOC UI: live alert queue, hybrid case investigation with reasoning stepper, MITRE coverage map, and detection editor.",
    tags: ["Next.js 16", "WebSocket", "MITRE ATT&CK"],
    icon: Cpu,
  },
  {
    title: "Threat Hunter Agent",
    description:
      "Natural-language hunt queries translated to HELIQL, executed against warm Iceberg history, and surfaced as annotated timelines.",
    tags: ["NL → HELIQL", "Iceberg hunt", "Timeline"],
    icon: Search,
  },
  {
    title: "Detection Engineer Agent",
    description:
      "Proposes HELIQL rules from natural language, shadow-deploys against the live stream, monitors drift, and opens a human-reviewed Git PR.",
    tags: ["NL → rule", "Shadow deploy", "PR review"],
    icon: GitPullRequest,
  },
  {
    title: "Responder Agent",
    description:
      "Policy-gated containment: isolate endpoint, revoke token, create ticket — each action signed, rate-limited, and human-approvable before execution.",
    tags: ["Containment", "SOAR bridge", "Signed actions"],
    icon: ShieldAlert,
  },
  {
    title: "AI-Threat Detection",
    description:
      "First-class detection of rogue internal AI: baseline MCP tool invocation, correlate OTEL GenAI traces with identity and egress, fire on anomalies.",
    tags: ["OTEL GenAI", "MCP abuse", "Agent identity"],
    icon: Fingerprint,
  },
  {
    title: "Enterprise & MSSP",
    description:
      "Multi-tenant isolation, BYOK encryption, federated push-down to Snowflake / Splunk, SSO, and compliance exports mapped to NIST / SOC 2 controls.",
    tags: ["Multi-tenant", "BYOK", "SOC 2"],
    icon: Database,
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
          From raw ingest to signed verdicts — each capability is a discrete, composable layer.
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

            {/* Title */}
            <h3 className="relative z-10 mt-4 text-sm font-semibold leading-snug">
              {feat.title}
            </h3>

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
