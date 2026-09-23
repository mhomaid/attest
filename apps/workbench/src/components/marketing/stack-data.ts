export const REPO_URL = "https://github.com/mhomaid/attest";

export type Status = "live" | "planned";

export type PlaneComponent = {
  name: string;
  tech: string;
  role: string;
  path?: string;
  status?: Status;
};

export type Plane = {
  id: string;
  name: string;
  summary: string;
  group: "control" | "data";
  components: PlaneComponent[];
};

export const repoLink = (path: string) => `${REPO_URL}/tree/main/${path}`;

export const isPlanned = (c: PlaneComponent) => c.status === "planned";

/** Top to bottom, as drawn in docs/02_Architecture.md §2. */
export const planes: Plane[] = [
  {
    id: "workbench",
    name: "Workbench plane",
    summary: "Where analysts work. Reads the same artifacts; keeps no second copy.",
    group: "control",
    components: [
      {
        name: "Workbench",
        tech: "Next.js 16 · Bun · Tailwind v4",
        role: "Queue, cases, hunt, Simulate and Load labs, and this site. Zustand, Better Auth.",
        path: "apps/workbench",
      },
      {
        name: "Control-plane API",
        tech: "Rust · axum 0.8 · utoipa",
        role: "REST and WebSocket API for events, baselines, warm SQL and fired detections.",
        path: "crates/attest-control-plane",
      },
      {
        name: "ws-gateway",
        tech: "Rust · tokio",
        role: "Streams an agent's reasoning trace to the case view as it runs.",
        path: "apps/ws-gateway",
      },
    ],
  },
  {
    id: "agentic",
    name: "Agentic plane",
    summary: "Agents decide and sign. They never open a database or a credential.",
    group: "data",
    components: [
      {
        name: "Orchestrator",
        tech: "Rust · tokio",
        role: "Hybrid loop: classify, escalate when novel, sign the result.",
        path: "crates/attest-orchestrator",
      },
      {
        name: "Triager classifier",
        tech: "XGBoost · tract-onnx",
        role: "Feature extractor plus ONNX model in-process, under 5 ms, with SHAP-style attributions.",
        path: "crates/attest-onnx-runtime",
      },
      {
        name: "Calibration sidecar",
        tech: "Python · uv",
        role: "Isotonic calibration and novelty parameters; training harness.",
        path: "ml/triager",
      },
      {
        name: "Inference router",
        tech: "Anthropic · OpenAI-compatible",
        role: "One LLM client for the escalation path: Claude, or a local model server.",
        path: "crates/attest-inference-router",
      },
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
      {
        name: "Attestation + attest CLI",
        tech: "ed25519-dalek · SHA-256",
        role: "Signed, hash-chained envelopes; verify, replay and pin-check offline.",
        path: "crates/attest-attestation",
      },
      {
        name: "Hunter · Responder · Coordinator",
        tech: "Agents",
        role: "Further agent roles. Policies exist; the agents are design only.",
        status: "planned",
      },
    ],
  },
  {
    id: "detection",
    name: "Detection plane",
    summary: "Write a rule once; compile it to the stream or to a hunt.",
    group: "data",
    components: [
      {
        name: "HELIQL compiler",
        tech: "Rust · pest",
        role: "Detection language compiled to RisingWave materialized views. Imports Sigma.",
        path: "crates/attest-heliql",
      },
      {
        name: "Detection runtime",
        tech: "Rust",
        role: "Deploys compiled rules and publishes fired alerts to Kafka.",
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
    id: "storage",
    name: "Storage plane",
    summary: "Open formats you own. Leave with your data in a day.",
    group: "data",
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
        name: "Cold archive",
        tech: "S3 Glacier",
        role: "Seven-year compliance retention.",
        status: "planned",
      },
    ],
  },
  {
    id: "streaming",
    name: "Streaming plane",
    summary: "Raw logs in, normalized OCSF on a bus within seconds.",
    group: "data",
    components: [
      {
        name: "Edge collector",
        tech: "Rust · axum 0.8",
        role: "CloudTrail JSON in, OCSF 1.3 out.",
        path: "crates/attest-collector",
      },
      {
        name: "Kafka / Redpanda",
        tech: "rdkafka 0.36",
        role: "Event bus, one topic per source, partitioned by tenant.",
        path: "docker-compose.yml",
      },
      {
        name: "RisingWave",
        tech: "Streaming SQL",
        role: "Recent events and 30-day entity baselines as materialized views.",
        path: "infra/risingwave",
      },
      {
        name: "Arroyo",
        tech: "Streaming SQL",
        role: "Multi-step sequences (login then S3 access) and Parquet ETL.",
        path: "infra/arroyo",
      },
      {
        name: "Okta · M365 · EDR collectors",
        tech: "OCSF 1.3",
        role: "More sources on the same normalizer.",
        status: "planned",
      },
    ],
  },
  {
    id: "control",
    name: "Control plane",
    summary: "Identity, config and infrastructure. Everything is in Git.",
    group: "control",
    components: [
      {
        name: "Identity + sessions",
        tech: "Better Auth · Postgres 18",
        role: "Analyst accounts and sessions; Alembic migrations.",
        path: "infra/db",
      },
      {
        name: "GitOps",
        tech: "GitHub Actions · Dependabot",
        role: "Detections, agent definitions and pipelines are versioned and CI-tested.",
        path: ".github",
      },
      {
        name: "Infrastructure",
        tech: "Docker Compose · Railway",
        role: "One compose file locally, one Railway project in the cloud.",
        path: "infra/railway",
      },
      {
        name: "Observability",
        tech: "OpenTelemetry · Tempo · Sentry",
        role: "Traces across Rust services; frontend errors with PII stripped.",
        path: "crates/attest-telemetry",
      },
      {
        name: "Enterprise identity",
        tech: "OIDC · SCIM",
        role: "SSO and provisioning via Okta, Entra or Auth0.",
        status: "planned",
      },
      {
        name: "Per-tenant keys",
        tech: "BYOK · KMS",
        role: "Customer-held encryption keys per tenant.",
        status: "planned",
      },
      {
        name: "Terraform + Helm",
        tech: "EKS · GKE · AKS",
        role: "Same containers deployed into your cloud or an air-gapped site.",
        status: "planned",
      },
    ],
  },
];

export type Deployment = {
  id: string;
  name: string;
  status: Status;
  controlIn: "attest" | "customer";
  dataIn: "attest" | "customer";
  note: string;
};

/** docs/02_Architecture.md §10 — product names, not the host vendor. */
export const deployments: Deployment[] = [
  {
    id: "self",
    name: "Run it yourself",
    status: "live",
    controlIn: "customer",
    dataIn: "customer",
    note: "Today: clone the repo and run all six planes on Docker with make dev-up-all. Single tenant, for evaluation, not production.",
  },
  {
    id: "hosted",
    name: "We host it",
    status: "planned",
    controlIn: "attest",
    dataIn: "attest",
    note: "Not offered yet. There is one single-tenant demo stack, usually paused to save cost. Your events would land in open formats you can take with you.",
  },
  {
    id: "byoc",
    name: "Your cloud",
    status: "planned",
    controlIn: "attest",
    dataIn: "customer",
    note: "We keep identity and the console. The data plane — ingest, detections, storage, agents — runs in your VPC. Your events never leave your cloud.",
  },
  {
    id: "airgap",
    name: "Air-gapped",
    status: "planned",
    controlIn: "customer",
    dataIn: "customer",
    note: "All six planes on your hardware. Open-weights models, no outbound calls.",
  },
];
