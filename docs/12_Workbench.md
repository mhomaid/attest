# 12 — Workbench: Stack, UI/UX, and Flows

**Product:** Attest (working name: Attest)
**Document type:** Technical and design specification of the analyst-facing workbench. Read this when you sit down to scaffold the front end.

> **A note on this document.** This is the canonical specification for everything that runs in a browser. It supersedes any web-stack details mentioned briefly in `02_Architecture.md` §6, `07_Stack_Revised.md` §8, and `11_Repo_Structure.md`. Those documents are correct at the level they describe; this one fills in the rest.

---

## 1. The principle

A SIEM workbench is not a generic SaaS dashboard. The user is a SOC analyst who sits in this tool for **eight hours a day**, in a **dark room at 3 AM**, with **forty alerts in the queue** and a CISO asking why the breach wasn't caught. The UX has more in common with a Bloomberg terminal, an air traffic control console, or a flight management computer than with a marketing landing page or a Stripe dashboard.

Two consequences shape every decision below:

1. **Information density is a feature, not a flaw.** Whitespace that looks elegant in a Figma file becomes hostile when the analyst needs to see twelve fields without scrolling. We design for the working professional, not the screenshot.
2. **Latency is judgment.** A 200ms delay rendering a reasoning trace is the difference between "the agent caught it" and "the analyst loses the thread." Performance budgets in §7 are non-negotiable.

Everything else in this document follows from those two ideas.

## 2. Canonical stack

The stack inherits the author's Next.js skill defaults, with Attest-specific overrides where the product warrants them.

| Layer | Technology | Notes for Attest |
|---|---|---|
| **Framework** | Next.js (App Router) `16.x` | Server Components for shells; Client Components for live surfaces |
| **Language** | TypeScript `^5` | No `any`. Zod-derived types for shared schemas. |
| **React** | React `19.x` | |
| **Package manager** | Bun `1.3.x` | `bun install`, `bun run`, `bun add`; `bunx` instead of `npx` |
| **Server APIs** | **Rust (axum)** for the REST API and **Rust (tokio-tungstenite)** for the WebSocket gateway. **Not Next.js API routes** for hot paths. | Hot paths must be Rust per `07_Stack_Revised.md`. Next.js route handlers are reserved for thin browser-side proxies that need session cookies. |
| **DB access from Next.js** | **None directly.** All analyst data flows through the Rust REST/WS APIs. | Postgres is for control-plane state only; the analyst never touches it through Next.js. |
| **Auth** | Better Auth (`better-auth`) | Session-based; OIDC providers configured per tenant; SCIM for provisioning |
| **Server state** | TanStack Query (`@tanstack/react-query`) | All HTTP data fetching from Rust APIs |
| **Live state** | Native WebSocket subscriptions wrapped in TanStack Query observers | No `useEffect` + `useState` for live data |
| **Client state** | Zustand | Shared UI state across distant components (sidebar, panel layouts, command palette) |
| **Validation** | Zod | Form validation, API request/response schemas, env vars |
| **Tables** | TanStack Table (`@tanstack/react-table`) | Sorting, filtering, pagination, virtualization |
| **Virtualization** | TanStack Virtual | Required for alert queue and reasoning trace replay |
| **Styling** | Tailwind CSS `^4` | with `@tailwindcss/postcss` |
| **Components** | shadcn + Base UI (`@base-ui/react`) | CVA, clsx, tailwind-merge |
| **Theming** | `next-themes` | **Dark mode is the default**, not an option (see §3.1) |
| **Icons** | Lucide (`lucide-react`) | |
| **Toasts** | Sonner | |
| **Drawers / sheets** | Vaul | |
| **Charts** | Recharts | For coverage heatmap, calibration plots, volume trends |
| **Drag & drop** | `@dnd-kit` | For panel rearrangement, evidence pinning |
| **Code/syntax** | Shiki or CodeMirror 6 | HELIQL editor in the Detection Engineer surface |
| **Diff viewer** | `react-diff-viewer-continued` | For PR review of agent-proposed detections |
| **Date/time** | `date-fns` + `date-fns-tz` | All timestamps rendered in tenant timezone with absolute + relative tooltip |
| **Analytics** | **PostHog** | Client + server event tracking, session replay (with PII masking), feature flags |
| **Error tracking** | **Sentry** | Browser exceptions, performance traces, session replay correlated with PostHog |
| **Email** | Resend | Transactional emails (auth, audit-export delivery) |
| **Background jobs** | (Attest uses Rust services, not Inngest) | Workflow orchestration is in the Rust agent runtime, not Next.js |
| **Linting** | ESLint + `eslint-config-next` | + Prettier |

**Key deviations from the generic Next.js skill:**

- **Inngest is not in the stack.** Background jobs in Attest are agent runs orchestrated by the Rust orchestrator, not Next.js workflows.
- **Kysely is not in the Next.js side.** Postgres exists, but the front end never queries it directly. All analyst-facing data comes from Rust REST/WS endpoints.
- **Sentry is added** — non-negotiable for a security product, see §2.3.
- **Dark mode is default**, not a choice. See §3.1.

## 3. UI/UX principles for SOC analysts

These principles are non-negotiable. They are the difference between a workbench analysts adopt and a workbench analysts route around.

### 3.1 Dark mode is the default

A SOC console runs 24×7. Half its users are looking at it at 2 AM. Light mode is opt-in, not the other way around. The `<html>` tag ships with `class="dark"`; the theme toggle exists for daytime users but never inverts the default.

All custom colors live in CSS variables under `:root` (light) and `.dark` (dark). Severity colors must remain perceptible in both — never use bare red/green; use Tailwind's `red-500` / `emerald-500` mapped through tokens that adjust luminance per theme.

### 3.2 Information density is a feature

A SOC analyst's screen at peak hours holds: alert queue (30+ rows visible), case header, evidence panel, reasoning trace, related cases, agent status, and threat intel context. **All visible without scrolling**, on a 14-inch laptop. Whitespace decisions optimize for working efficiency, not screenshots.

Default Tailwind spacing is too generous. We use a tighter scale: `p-2` where shadcn defaults to `p-4`, `gap-2` where it defaults to `gap-4`. Custom tokens enforce this in `globals.css`.

### 3.3 Keyboard-first

Analysts who live in a SIEM use the keyboard. Every primary action has a keyboard shortcut. The shortcut overlay is one keypress away (`?`). The command palette (`⌘K`) reaches every action and every navigable surface.

Mouse-only interactions are an explicit decision, not a default. Drag-to-resize panels, pin-to-sticky, and color-picker selections are exceptions; everything else has a keyboard path.

### 3.4 Live updates without breaking focus

Real-time data streams in, but the analyst's current focus is sacred. New alerts in the queue do **not** auto-scroll the list. The case the analyst is reading does **not** re-render its evidence panel from under them. Updates surface as **subtle, peripheral signals** (badge count, gentle border highlight, optional toast) until the analyst chooses to refresh.

This is the single most violated UX principle in real-time security tools and is one of the easiest places to differentiate.

### 3.5 Immutable history

Cases, attestations, and detection deployments are append-only from the analyst's perspective. The verdict can be **overridden** (creating a new attestation that records the override + reason); the original verdict is never erased. Detection PRs can be **revoked** (creating a new commit); never force-pushed away. Every visible state has a permanent history accessible via the time-travel debugger or audit log.

### 3.6 No marketing motion

Animations exist only to signal state changes — a row entering the queue, a verdict being submitted, a panel opening. **No parallax, no scroll-triggered reveals, no decorative transitions.** Motion durations are 100–200ms; anything longer feels sluggish in a working tool. The `motion` library from the Next.js skill is used **only** in the public marketing site (the future `/` for marketing), never inside `/workbench` routes.

### 3.7 Error states tell the truth

A failed query, a stale calibration, a degraded LLM provider, an MCP gateway timeout — all surfaced **explicitly in-context**, not hidden behind a generic "something went wrong." Analysts need to know *what* failed so they can decide whether to retry, escalate, or work around it.

### 3.8 Confidence is shown, not hidden

Every agent verdict displays its **calibrated confidence** as a numeric value plus a visual indicator. SHAP attributions for classifier paths and citation density for LLM paths are first-class UI elements, not buried in a debug panel. The analyst sees not just *what* the agent decided but *how solid* the decision is.

## 4. Information architecture

### 4.1 Top-level routes

```
/                            # Public marketing landing (separate from app shell)
/login, /sign-up             # Auth
/workbench                   # Authenticated app shell — primary working surface
  /workbench/queue           # Alert queue (default landing for an analyst)
  /workbench/cases/[id]      # Case workbench (one case at a time, full focus)
  /workbench/hunt            # Threat-hunting surface (Hunter agent + analyst hypotheses)
  /workbench/detections      # Detection management (browse, edit, deploy HELIQL)
  /workbench/detections/prs  # Detection Engineer agent's open PRs awaiting review
  /workbench/coverage        # MITRE ATT&CK coverage heatmap
  /workbench/agents          # Per-agent governance console (calibration, policy, audit)
  /workbench/agents/[id]/replay/[action_id]  # Time-travel debugger for any past decision
/admin                       # Admin surface (tenant settings, user management, BYOK)
  /admin/audit               # Tenant audit log
  /admin/integrations        # MCP servers, SOAR connectors, threat intel feeds
  /admin/policy              # Per-agent policy as code (GA v1)
/settings                    # Per-user settings (theme, shortcuts, notifications)
```

### 4.2 Navigation primitives

- **Top bar:** tenant switcher, command palette trigger (`⌘K`), notification bell, user menu.
- **Left sidebar (collapsible):** queue, cases, hunt, detections, coverage, agents, admin. Persistent across `/workbench/*` routes.
- **Per-route action bar:** route-specific actions live at the top of the main content area. No floating action buttons.
- **Right rail (per-case):** evidence panel, related cases, threat intel context. Resizable; collapsible.

### 4.3 Entity model surfaced to the analyst

```
Alert ──→ Case ──┬──→ Triager Verdict (one)
                 ├──→ Investigator Verdict (zero or one)
                 ├──→ Hunter Findings (zero or many)
                 ├──→ Responder Actions (zero or many)
                 └──→ Attestation Envelopes (one per agent action)

Detection ──→ Backtest Result (many over time)
           ──→ Production Fires (many over time)
           ──→ MITRE Technique mappings (many)

AI Agent (governance entity, distinct from Attest's own agents)
       ──→ Behavior Baseline (continuously updated)
       ──→ AADF Detections fired against it (zero or many)
       ──→ MCP/A2A traces (continuously appended)
```

The IA exists so an analyst always knows where in the entity graph they are. URL paths reflect this; breadcrumbs make it explicit.

## 5. Critical user flows

The seven flows below define the product. Every other interaction is supporting infrastructure.

### 5.1 Flow A — Alert lands, classifier dispositions, analyst sees it

```
1. Alert appears on `alerts` Redpanda topic
2. Rust orchestrator routes to Triager classifier path
3. XGBoost runs in <50ms, produces verdict + SHAP + calibrated confidence
4. Attestation envelope (Classifier variant) signed and persisted
5. WS gateway pushes alert + verdict over the analyst's `/queue` subscription
6. Alert row renders in queue; if calibrated confidence ≥ threshold AND verdict
   is benign, row is auto-collapsed under "auto-closed (12)" group
7. Analyst sees the queue; auto-closed group is one click away to expand and audit
```

**SLA:** alert in queue UI within 200ms P99 of classifier verdict. Auto-closed cases never page; they appear in the queue as a low-noise group.

### 5.2 Flow B — Analyst opens a case, reasoning trace renders live

```
1. Analyst clicks an alert in the queue
2. Route transitions to /workbench/cases/[id] (Server Component shell)
3. Case header, evidence panel scaffold render immediately from SSR
4. Client Component subscribes to /ws/cases/[id]/trace via the WS gateway
5. If the case is in flight (Investigator running), trace steps stream in live
6. If the case is complete, trace replays from the attestation envelope
7. Each step renders as it arrives; analyst can scroll the trace without
   disrupting incoming steps (they queue at the top, never auto-scroll)
8. Verdict + calibrated confidence + path-specific evidence (SHAP or citations)
   render in the right rail
```

**SLA:** case shell TTI ≤ 1.5s; first trace step rendered ≤ 200ms after WS connection established.

### 5.3 Flow C — Analyst overrides a verdict

```
1. Analyst reviews trace; disagrees with verdict
2. Presses keyboard shortcut O (or clicks Override)
3. Modal: select corrected label + reason (free-text, mandatory)
4. Submit triggers POST to Rust REST API
5. New attestation envelope signed with override metadata; original preserved
6. Override flows back to calibration loop as ground-truth signal
7. Case state transitions to Closed (Human Override); UI reflects within 200ms
```

**Critical detail:** the override is itself attested. An auditor can replay both the original and the override.

### 5.4 Flow D — Analyst types a natural-language query

```
1. Analyst presses ⌘K, types "logins from new countries last 3 days"
2. Command palette routes to NL → HELIQL via the LLM Detection Engineer agent
3. Compiled HELIQL displayed for review (analyst can edit before running)
4. Run triggers federated execution; results stream into a TanStack Table
5. Each row links to the underlying OCSF event; clicking opens evidence
6. Save-as-detection button captures the HELIQL into the Detection Engineer's
   PR queue for review and shadow-deploy
```

**SLA:** NL → HELIQL compilation ≤ 3s; query results streaming to first row ≤ 1s after submission.

### 5.5 Flow E — Detection Engineer agent opens a PR; engineer reviews

```
1. Detection Engineer agent runs (continuous loop in Rust orchestrator)
2. Identifies coverage gap, drafts HELIQL, runs backtest, opens GitHub PR
3. PR appears at /workbench/detections/prs with backtest precision/recall
4. Engineer opens PR in workbench (renders unified diff + backtest report)
5. Engineer can: approve (merges + shadow-deploys), request changes (returns
   to agent with feedback), or reject (closes PR, signals retraining)
6. Approved PR auto-promotes to production after 7-day shadow window if
   precision and recall thresholds hold
```

**Critical detail:** the engineer's review feedback becomes training signal for the agent. Rejection without feedback is allowed but discouraged — a banner reminds the engineer that feedback improves future PRs.

### 5.6 Flow F — Auditor exports a signed reasoning trace

```
1. Auditor (read-only role) opens any case in /workbench/cases/[id]
2. Clicks Export → Signed Audit Bundle
3. Workbench API gathers all attestation envelopes for the case + referenced
   evidence (OCSF events, tool call results, model versions)
4. Bundle signed with tenant-specific key and made available as encrypted
   ZIP delivered via Resend email or downloaded directly
5. Auditor verifies offline using the standalone attestation-verifier CLI
   (defined in 11_Repo_Structure.md tools/attestation-verifier/)
6. Bundle is itself logged in the audit log with the auditor's identity
```

**Critical detail:** signed audit bundles work offline. An auditor in a SOC review meeting without internet can verify Attest decisions on a laptop.

### 5.7 Flow G — Live agent reasoning observed from outside the case

```
1. Analyst on the queue page sees a case with status "Investigator running"
2. Hovers over the status pill; tooltip shows live progress (current step,
   tools used so far, latest intermediate belief summary)
3. Click navigates to the case with the WS subscription already opening
4. Trace continues seamlessly; the analyst doesn't re-establish context
```

**Why this matters:** in a real SOC, an analyst supervising autonomous work needs to know *at a glance* what the agents are doing without context-switching into the case.

## 6. Component inventory

The fifteen components below are the non-trivial ones — anything not listed is shadcn defaults or thin wrappers.

| Component | Purpose | Notes |
|---|---|---|
| `AlertQueue` | Virtualized list of alerts grouped by state (in-flight, awaiting review, auto-closed) | TanStack Virtual; live WS subscription; sticky header |
| `CaseWorkbench` | Top-level surface for a single case | Server Component shell + Client Component panels |
| `ReasoningTraceViewer` | Renders any of the three envelope variants | Switches presentation by `execution_path`; see below |
| `ClassifierEvidencePanel` | SHAP feature attribution as a horizontal bar plot | Recharts; sortable by impact |
| `LLMReasoningTimeline` | Step-by-step chain of tool calls + intermediate beliefs | Virtualized; expandable steps; citation hover-cards |
| `HybridDualEvidence` | Side-by-side classifier draft + LLM final | Used when envelope is `hybrid` variant |
| `EvidencePanel` | Pinned OCSF events, asset context, threat intel | Drag-to-pin; collapsible groups |
| `MitreCoverageHeatmap` | Tactic × Technique grid with detection counts | Recharts; click drills into detections per technique |
| `AgentGovernanceConsole` | Per-agent calibration, policy, recent decisions | Tabbed; published Brier score and ECE per case-class |
| `CalibrationReliabilityPlot` | Reliability diagram per agent per execution path | Recharts; published to customer in audit-export bundles |
| `HELIQLEditor` | Code editor with HELIQL syntax + validation | CodeMirror 6 + Shiki for highlighting |
| `DetectionPRReview` | Unified diff + backtest report + approve/request-changes | react-diff-viewer-continued |
| `CommandPalette` | ⌘K surface for actions and navigation | cmdk-style; routes through every entity type |
| `TimeTravelDebugger` | Replays any past attestation envelope step-by-step | Reads envelope from API; reconstructs trace UI |
| `AgentLiveStatusPill` | Compact status indicator for in-flight agent runs | Tooltip with live progress |

### 6.1 The reasoning trace is three components, not one

Because attestation envelopes have three variants, the trace viewer has three matched components. The parent `ReasoningTraceViewer` switches based on `envelope.execution_path`:

- `execution_path: "classifier"` → renders `ClassifierEvidencePanel` only
- `execution_path: "llm"` → renders `LLMReasoningTimeline` only
- `execution_path: "hybrid"` → renders `HybridDualEvidence` (which embeds both)

This is a load-bearing UX moment: it is **the visible proof of Attest's hybrid posture**. Most agentic-SOC competitors render an opaque LLM trace; we render the calibrated classifier evidence next to the LLM reasoning when both ran.

## 7. State commitments — every component handles four

Every non-trivial component must explicitly handle:

1. **Loading state** — skeleton UI matching the final layout dimensions; never a spinner alone, never layout shift.
2. **Empty state** — explanatory copy + the next action the analyst can take. Never "No data."
3. **Error state** — what failed, what the user can do, a retry button when retry is meaningful. Never a generic toast.
4. **Edge state** — partial data, degraded mode, stale calibration, slow LLM provider. Surfaced explicitly with severity-appropriate styling.

Loading states are owned by Next.js `loading.tsx` files in each route group. Error states are owned by `error.tsx`. Empty and edge states are component-local concerns.

### 7.1 Specific edge states the workbench must handle

- **Stale calibration:** an agent's calibration model is older than 7 days. Render a small `⚠ stale calibration` badge next to confidence numbers.
- **LLM provider degraded:** Anthropic API is rate-limited. Render a banner offering to fall back to local Qwen with explicit user consent.
- **Iceberg query slow:** warm-tier query exceeded 30s. Render a progress indicator and let the user cancel without losing the query draft.
- **WS disconnected:** WebSocket gateway lost connection. Render a thin red bar; auto-reconnect with exponential backoff; queue analyst actions until reconnect.
- **Attestation signature invalid:** an attestation envelope failed verification. Render the case with a high-severity warning banner; do not auto-close.

## 8. Performance budget

Non-negotiable targets, measured via PostHog and Sentry performance monitoring (§9).

| Surface | Target P50 | Target P99 |
|---|---|---|
| Time to interactive — `/workbench/queue` | 800ms | 1.5s |
| Time to interactive — `/workbench/cases/[id]` | 800ms | 1.5s |
| Alert row render after WS push | 50ms | 200ms |
| Reasoning trace step render after WS push | 30ms | 100ms |
| Time-travel debugger scrub between steps | 16ms (60fps) | 50ms |
| Command palette open after `⌘K` | 30ms | 100ms |
| HELIQL editor keystroke → syntax check | 16ms | 50ms |
| Coverage heatmap render (full MITRE matrix) | 200ms | 500ms |

Performance regressions exceeding the P99 budget block the release in CI.

## 9. Observability — PostHog and Sentry

A security product cannot ship without rigorous observability of its own UI. Two providers, distinct purposes, integrated.

### 9.1 PostHog — product analytics + session replay + feature flags

- **Pageviews and events:** manual tracking via `usePathname()` + `useSearchParams()` because App Router doesn't auto-track. Every analyst action emits a typed event (case opened, verdict overridden, query run, PR approved).
- **Session replay:** enabled with **strict PII masking** by default. All input fields, all OCSF event payloads, all customer data are masked unless explicitly tagged safe. The analyst's interactions with the UI shell are visible; no customer data leaves the customer's browser to PostHog.
- **Feature flags:** every new analyst-facing surface ships behind a flag. Rollout: 1 design partner → 3 → all. Rollback is one toggle.
- **Cohorts:** per-tenant, per-role (Analyst / Detection Engineer / Auditor / Admin), per-trust-rung (where the customer is on the autonomy ladder from `03_AI_Agentic_Strategy.md` §8).

```typescript
// src/lib/posthog.ts
import posthog from "posthog-js";

export function initPostHog() {
  if (typeof window === "undefined") return;
  posthog.init(process.env.NEXT_PUBLIC_POSTHOG_KEY!, {
    api_host: process.env.NEXT_PUBLIC_POSTHOG_HOST || "https://us.i.posthog.com",
    capture_pageview: false,
    session_recording: {
      maskAllInputs: true,
      maskTextSelector: '[data-customer-data]',  // anything tagged is masked
    },
  });
}
```

**Critical commitment:** customer data — OCSF events, agent reasoning content, tool call results — is **never** transmitted to PostHog. Only UI-shell events and metadata. This is enforced by the masking config and by code review on any new event emitter.

### 9.2 Sentry — error tracking + performance monitoring

- **Browser exceptions:** every unhandled error caught and reported with full stack trace and breadcrumbs.
- **Performance monitoring:** Web Vitals (LCP, FID, CLS, INP) tracked per route. Custom transactions for the seven critical flows in §5.
- **Release tracking:** source maps uploaded per release; errors attributed to the specific commit.
- **Replay correlation:** Sentry replay correlates with PostHog session ID via a shared `session_id` attribute. Same masking rules apply.
- **Alerting:** P0 errors page on-call within 5 minutes. P99 budget violations open weekly review tickets.

```typescript
// sentry.client.config.ts
import * as Sentry from "@sentry/nextjs";

Sentry.init({
  dsn: process.env.NEXT_PUBLIC_SENTRY_DSN,
  tracesSampleRate: 0.1,
  replaysSessionSampleRate: 0.05,
  replaysOnErrorSampleRate: 1.0,
  integrations: [
    Sentry.replayIntegration({
      maskAllInputs: true,
      maskAllText: false,  // UI shell text is fine; customer data is in masked elements
      blockAllMedia: false,
    }),
  ],
  beforeSend(event) {
    // Strip any field tagged as customer data
    return scrubCustomerData(event);
  },
});
```

**Same critical commitment as PostHog:** customer data never reaches Sentry. The `beforeSend` hook scrubs anything tagged. Code review on every error context to ensure no PII or OCSF payloads leak.

### 9.3 The observability split

| Concern | Tool |
|---|---|
| Did the analyst use this feature? | PostHog |
| Was the analyst surprised by something? | PostHog session replay |
| Did the page render correctly? | Sentry Web Vitals |
| Did the page error out? | Sentry exceptions |
| Is this surface meeting its P99 budget? | Sentry transactions + PostHog events |
| Should we ship this surface to all customers? | PostHog feature flags |

Both providers are **strictly opt-in for regulated tenants** — a healthcare customer in BYOC can disable both via a tenant config flag. Their UI still works; Attest just doesn't observe it.

## 10. Accessibility

WCAG 2.1 AA, not as a checklist but as a working commitment. Specifically for the SOC analyst context:

- **Keyboard navigation everywhere.** No focus traps. Tab order matches visual order. Skip links on every route.
- **Screen reader support** for the alert queue and reasoning trace — analysts with low vision are a real audience in government and healthcare.
- **Reduced motion** honored via `prefers-reduced-motion`. All entrance animations disable; functional state-change animations remain.
- **Color is not the only signal.** Severity uses color **and** an icon **and** a text label. A red-green colorblind analyst loses no information.
- **Minimum contrast 4.5:1** for body text in both light and dark themes. Severity colors tested in both.

## 11. What's intentionally NOT in the workbench

Honest scope discipline saves engineering time and protects credibility:

- **No marketing animations or scroll-triggered effects** on `/workbench/*` routes. Save those for `/`.
- **No customizable dashboards as a v1 feature.** Analysts asked for them in every prior SIEM and used them less than 5% of the time. Defer to GA v1+ if real demand emerges.
- **No mobile-optimized analyst experience.** The workbench is a desktop tool. A read-only mobile view of cases is a Year 2 consideration.
- **No light-mode-first design reviews.** Dark mode is the working default; light mode is opt-in. Designs are reviewed in dark mode first.
- **No marketing-style empty states with illustrations.** Empty states are functional copy + next action.
- **No "AI sparkle" effects.** No glittering gradients on AI-driven UI elements. The agentic capabilities are differentiated by *substance* (calibrated confidence, signed attestation), not by visual hype.
- **No Inngest or Next.js API route–driven workflows.** All workflow orchestration is in the Rust agent runtime.
- **No direct database access from Next.js.** All data flows through the Rust REST/WS APIs.

## 12. The bar for "the workbench is real"

A new contributor can run, in this order, in under 30 minutes:

1. `bun install` → `bun run dev` brings up Next.js against the local docker-compose stack.
2. `/workbench/queue` renders with seeded sample alerts within 1.5s TTI.
3. Opening any sample case streams a reasoning trace over WebSocket.
4. ⌘K opens the command palette and routes to every navigable surface.
5. Verdict override on any case round-trips into the orchestrator and persists.
6. PostHog records the override event; Sentry shows zero unhandled exceptions.
7. Theme toggle works without flicker; all surfaces render in both light and dark.
8. Lighthouse score ≥ 90 on the queue page in dark mode.

Hitting all eight is the definition of done for the MVP workbench (Phase 8 in `10_Build_Order.md`).

## 13. The single sentence

> **The workbench is the eight-hours-a-day surface that makes Attest's verifiable agentic posture visible to the people who actually defend the network. It is information-dense, dark-mode-first, keyboard-driven, latency-aware, and rigorously observable — a working tool, not a screenshot.**
