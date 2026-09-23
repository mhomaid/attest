export const REPO_URL = "https://github.com/mhomaid/attest";

export type StackComponent = {
  name: string;
  tech: string;
  role: string;
  path: string;
};

export type StackLayer = {
  id: string;
  name: string;
  summary: string;
  components: StackComponent[];
};

export const repoLink = (path: string) => `${REPO_URL}/tree/main/${path}`;

export const stackLayers: StackLayer[] = [
  {
    id: "ingest",
    name: "Ingest",
    summary: "Raw vendor logs in, one normalized schema out.",
    components: [
      {
        name: "attest-collector",
        tech: "Rust · axum 0.8",
        role: "Accepts CloudTrail JSON and normalizes it to OCSF. Okta and M365 are next.",
        path: "crates/attest-collector",
      },
      {
        name: "OCSF 1.3",
        tech: "attest-common",
        role: "Shared event types every other service speaks.",
        path: "crates/attest-common",
      },
      {
        name: "load-gen",
        tech: "Rust",
        role: "Replays benign and attack traffic for load and demo runs.",
        path: "tools/load-gen",
      },
    ],
  },
  {
    id: "stream",
    name: "Stream",
    summary: "Every event is on a bus and in streaming SQL within seconds.",
    components: [
      {
        name: "Kafka / Redpanda",
        tech: "rdkafka 0.36",
        role: "Event bus, one topic per source, partitioned by tenant.",
        path: "docker-compose.yml",
      },
      {
        name: "RisingWave",
        tech: "Streaming SQL",
        role: "Hot tier: recent events and 30-day entity baselines as materialized views.",
        path: "infra/risingwave",
      },
      {
        name: "Arroyo",
        tech: "Streaming SQL",
        role: "Multi-step sequences (login then S3 access) and Parquet ETL.",
        path: "infra/arroyo",
      },
    ],
  },
  {
    id: "detect",
    name: "Detect",
    summary: "Rules are code: one language, compiled to the stream.",
    components: [
      {
        name: "HELIQL",
        tech: "Rust · pest",
        role: "Detection language compiled to RisingWave materialized views. Imports Sigma.",
        path: "crates/attest-heliql",
      },
      {
        name: "detection-runtime",
        tech: "Rust",
        role: "Deploys compiled rules and publishes fired alerts.",
        path: "crates/attest-detection-runtime",
      },
      {
        name: "10 detections",
        tech: "AWS · Okta · M365",
        role: "MITRE-mapped reference rules in the repo. AWS rules run live today.",
        path: "detections",
      },
    ],
  },
  {
    id: "store",
    name: "Store",
    summary: "Open formats you own. No proprietary copy of your data.",
    components: [
      {
        name: "Apache Iceberg + Parquet",
        tech: "iceberg 0.9 · arrow 58",
        role: "Warm tier on MinIO locally, S3 in production.",
        path: "crates/attest-storage-iceberg",
      },
      {
        name: "ClickHouse",
        tech: "s3() over Parquet",
        role: "Hunting queries over the same warm files, no ETL copy.",
        path: "crates/attest-storage-clickhouse",
      },
      {
        name: "Postgres 18",
        tech: "Alembic migrations",
        role: "Sessions, users and platform state.",
        path: "infra/db",
      },
    ],
  },
  {
    id: "decide",
    name: "Decide",
    summary: "A fast classifier for known patterns, an LLM only for novel ones.",
    components: [
      {
        name: "Feature extractor",
        tech: "Rust",
        role: "Turns an alert into the classifier's fixed feature vector.",
        path: "crates/attest-feature-extractor",
      },
      {
        name: "ONNX classifier",
        tech: "XGBoost · tract-onnx",
        role: "Trained in Python, served in-process in under 5 ms with SHAP values.",
        path: "crates/attest-onnx-runtime",
      },
      {
        name: "Orchestrator",
        tech: "Rust · tokio",
        role: "Runs the hybrid loop: classify, escalate when novel, sign the result.",
        path: "crates/attest-orchestrator",
      },
      {
        name: "Inference router",
        tech: "Claude Sonnet 4.5",
        role: "One client for LLM calls on the escalation path.",
        path: "crates/attest-inference-router",
      },
    ],
  },
  {
    id: "govern",
    name: "Govern",
    summary: "The model never touches data or credentials directly.",
    components: [
      {
        name: "MCP gateway",
        tech: "Rust · axum",
        role: "The only door for agent tool calls. Inspects requests and results.",
        path: "crates/attest-mcp-gateway",
      },
      {
        name: "Policy engine",
        tech: "Rust",
        role: "Per-role tool allowlists, confidence floors, exfil caps, injection checks.",
        path: "crates/attest-policy-engine",
      },
      {
        name: "Shadow check",
        tech: "Rust",
        role: "Deterministic second opinion before any auto-close.",
        path: "crates/attest-shadow-check",
      },
    ],
  },
  {
    id: "prove",
    name: "Prove",
    summary: "Every verdict is signed, chained and replayable offline.",
    components: [
      {
        name: "Attestation envelopes",
        tech: "ed25519-dalek · SHA-256",
        role: "Signed, hash-chained record of every agent decision.",
        path: "crates/attest-attestation",
      },
      {
        name: "attest CLI",
        tech: "Rust · clap",
        role: "verify, replay and pin-check a log with no running services.",
        path: "crates/attest-cli",
      },
      {
        name: "Agent definitions",
        tech: "JSON + prompts",
        role: "Published model and prompt hashes that envelopes are pinned to.",
        path: "agents",
      },
    ],
  },
  {
    id: "serve",
    name: "Serve",
    summary: "APIs and the analyst console on top of the same artifacts.",
    components: [
      {
        name: "Control plane",
        tech: "axum 0.8 · utoipa",
        role: "REST and WebSocket API with OpenAPI docs.",
        path: "crates/attest-control-plane",
      },
      {
        name: "ws-gateway",
        tech: "Rust",
        role: "Streams an agent's reasoning trace to the case view as it runs.",
        path: "apps/ws-gateway",
      },
      {
        name: "Workbench",
        tech: "Next.js 16 · Bun · Tailwind v4",
        role: "Analyst console and this site. Zustand, Better Auth, framer-motion.",
        path: "apps/workbench",
      },
    ],
  },
  {
    id: "operate",
    name: "Operate",
    summary: "Traced, monitored and shipped from one repo.",
    components: [
      {
        name: "OpenTelemetry + Tempo",
        tech: "OTLP",
        role: "Traces across every Rust service.",
        path: "crates/attest-telemetry",
      },
      {
        name: "Sentry + PostHog",
        tech: "Errors · product analytics",
        role: "Frontend errors and usage, with PII stripped.",
        path: "apps/workbench",
      },
      {
        name: "Docker + Railway",
        tech: "Compose · Railway",
        role: "One compose file locally, one Railway project in the cloud.",
        path: "infra/railway",
      },
      {
        name: "GitHub Actions",
        tech: "CI · Dependabot",
        role: "Rust tests, lint and Playwright on every push; weekly dependency updates.",
        path: ".github",
      },
    ],
  },
];
