# Attest — design documentation

This directory holds the design specification for Attest, a streaming-first security operations platform with a verifiable agentic SOC.

> **Design documents describe the target architecture, not the current state.** For what is implemented today, see the Status table in the [root README](../README.md). Where a document and the code disagree, the code is the source of truth for current behavior and the document is the source of truth for intent. Either way, the mismatch is worth an issue.

## The thesis in one paragraph

AI agents are now both a new class of attacker and a new class of defender, and security platforms have to handle both. Attest is a streaming-first, composable security operations platform built around three capabilities:

- **Agent-Aware Detection Fabric.** It treats AI agents as first-class entities for detection.
- **Verifiable Agentic SOC.** Every autonomous decision is cryptographically attested, confidence-calibrated, and shadow-checked against deterministic rules.
- **Self-Improving Detection Mesh.** Specialized agents author, test, deploy, tune, and retire detections under human governance.

## Reading order

| # | Document | What it answers |
|---|---|---|
| 01 | [Product Requirements](./01_PRD.md) | What is being built, for whom, and the functional requirements |
| 02 | [Technical Architecture](./02_Architecture.md) | The six-plane model, data flow, and deployment topology |
| 03 | [AI & Agentic Strategy](./03_AI_Agentic_Strategy.md) | Why the agentic plane is designed the way it is |
| 03 | [Architecture Diagrams](./03_Architecture_Diagrams.md) | Mermaid diagrams, from broadest to most detailed |
| 07 | [Tech Stack](./07_Stack_Revised.md) | All-Rust services, Redpanda, MinIO/Iceberg, ClickHouse, Next.js. Supersedes the stack notes in 02 |
| 08 | [Datasets & ML](./08_Datasets_and_ML.md) | Which data and models are used, and what is trained versus not |
| 09 | [Agent Harness](./09_Agent_Harness.md) | Execution paths, the attestation envelope, calibration, and evaluation |
| 10 | [Build Order](./10_Build_Order.md) | Implementation sequence, with an end-to-end test per feature |
| 11 | [Repo Structure](./11_Repo_Structure.md) | Top-level repository layout |
| 12 | [Workbench](./12_Workbench.md) | Analyst workbench: stack, UI/UX principles, flows, observability |
| 15 | [Streaming Engine ADR](./15_Streaming_Engine_Decision.md) | Arroyo vs. Flink vs. RisingWave, with revisit triggers |
| — | [Phase chronicle](./phases.md) | What actually shipped in Phases 1–8 (moved from the root README) |

Gaps in the numbering are intentional; numbers are stable identifiers, not a sequence.

## Reading paths

- **Evaluating the architecture:** 07 → 02 → 09 → 15
- **About to write backend code:** 07 → 09 → 10 → 11
- **About to write frontend code:** 12 → 07 (§8) → 11 (`apps/workbench/`)
- **Interested in the ML and verification story:** 08 → 09 → 03

## Conventions

- **Later documents supersede earlier ones where they differ.** Doc 07 supersedes the stack table in doc 02. Docs 08 and 09 supersede the LLM-only Triager implied in doc 03 with the hybrid classifier-first design. Earlier documents are kept for context.
- **Numbers are design targets.** Volumes, latencies, and costs are targets unless a benchmark in the repo backs them.
- **Vendor names map the landscape.** References to other products describe where Attest fits, not claims about their internals.
- **Architecture changes get an ADR.** File them in [`decisions/`](./decisions/).
