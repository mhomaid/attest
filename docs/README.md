# Attest

**Working title** for an AI-agentic, streaming-first security operations platform. Author: Mohamed Homaid.

This package is a complete product blueprint — written as if Attest were a real internal program at a Series A security company. It exists so a reader can evaluate three things in one sitting:

1. Whether the author understands the streaming-first SIEM space at the level of someone who has built data platforms at scale.
2. Whether the proposed product actually solves a real, current, unsolved problem.
3. Whether the technical and organizational plan is credible.

## The thesis in one paragraph

The next generation of SIEM is not "AI-bolted-on streaming SIEM." It is the recognition that **AI agents are simultaneously the most dangerous new class of attacker and the most powerful new class of defender**, and that no current platform — including the strongest streaming-first players — is purpose-built for that reality. Attest is a streaming-first, composable security operations platform with three first-of-kind capabilities: an **Agent-Aware Detection Fabric** that treats AI agents as first-class entities for detection; a **Verifiable Agentic SOC** where every autonomous decision is cryptographically attested, confidence-calibrated, and shadow-checked against deterministic rules; and a **Self-Improving Detection Mesh** where specialized agents continuously author, test, deploy, tune, and retire detections under human governance.

## Documentation

The `docs/` directory is the canonical specification of HELIX/Attest. Read it
before contributing.

- Start: `docs/README.md`
- For new engineers: `docs/02_Architecture.md` → `docs/07_Stack_Revised.md` → `docs/09_Agent_Harness.md`
- For frontend engineers: `docs/12_Workbench.md`
- For PRs that change architecture: file an ADR in `docs/decisions/`

When code and docs disagree, that is a bug. Open an issue.

## Reading order

| # | Document | What it answers |
|---|---|---|
| 00 | [Executive Summary](./00_Pitch.md) | Why this, why now, what we win on |
| 01 | [Product Requirements Document](./01_PRD.md) | What we're building and for whom |
| 02 | [Technical Architecture](./02_Architecture.md) | How it's built (original) |
| 03 | [AI & Agentic Strategy](./03_AI_Agentic_Strategy.md) | How AI is woven through every layer |
| 04 | [Competitive Differentiation](./04_Differentiation.md) | Where we beat Abstract, Microsoft, Google, Splunk, Databricks |
| 05 | [MVP & Roadmap](./05_Roadmap.md) | What ships in 90 days, 6 months, 12 months |
| 06 | [Go-to-Market & Positioning](./06_GTM.md) | How we sell it |
| 07 | [Revised Tech Stack](./07_Stack_Revised.md) | All-Rust, Redpanda, MinIO, Railway, Next.js — supersedes stack notes in 02 |
| 08 | [Datasets & ML Strategy](./08_Datasets_and_ML.md) | What data we use, what models we use, what we train |
| 09 | [Agent Harness](./09_Agent_Harness.md) | Technical spec of the Verifiable Agentic SOC runtime |
| 10 | [Build Order](./10_Build_Order.md) | The 90-day implementation sequence with E2E tests for every feature |
| 11 | [Repo Structure](./11_Repo_Structure.md) | Top-level repository layout when you sit down to `git init` |
| 12 | [Workbench](./12_Workbench.md) | Workbench stack, UI/UX principles for SOC analysts, flows, components, PostHog + Sentry observability |

## Recommended reading paths

**For Abstract Security leadership (interviewer pitch):**
00 → 04 → 02 + 07 → 03

**For an engineer evaluating credibility:**
07 → 02 → 09 → 10

**For an investor or board:**
00 → 04 → 05 → 06

**For someone about to start writing code:**
07 → 09 → 10 → 11 → 12

**For a frontend engineer scaffolding the workbench:**
12 → 07 (§8) → 11 (apps/workbench/) → 10 (Phase 8)

## Conventions used

- **Attest** is a placeholder codename. Rename freely.
- Where this document references real companies (Abstract Security, Microsoft Sentinel, Splunk, etc.), it does so to map the competitive landscape, not to disparage. Abstract Security in particular has built the strongest streaming-first foundation in the market; Attest is positioned as the *next architectural layer above what they've built*, not a replacement for it.
- All technical claims (volumes, latencies, costs, frameworks) are stated as design targets, not current state.
- **Documents 07, 08, and 09 supersede earlier documents where they differ.** Specifically: doc 07 supersedes the tech stack table in doc 02; docs 08 and 09 supersede the Triager-as-LLM design implied in doc 03 with the hybrid classifier-first design. Earlier documents are preserved in their original form for context, but the canonical decisions live in the later docs.
