<wizard-report>
# PostHog post-wizard report

The wizard has completed a deep integration of PostHog analytics into the Attest Workbench. The setup follows the Next.js 15.3+ best practice of initializing PostHog in `instrumentation-client.ts`, which runs automatically before the app renders. A reverse proxy (`/ingest/*`) was added to `next.config.ts` to route PostHog requests through the Next.js server, improving ad-blocker resilience. All PII masking rules from the original design are preserved: text inputs are masked in session replay, `[data-customer-data]` elements are redacted, and blocked keys (`email`, `alert_body`, `case_body`, `reasoning`) are stripped from all properties. Exception capture is enabled via `capture_exceptions: true`. User identification is performed on successful login using the user's opaque ID (never email).

| Event | Description | File |
|---|---|---|
| `user_signed_in` | Analyst successfully signs in; also calls `posthog.identify()` with the user ID | `src/app/(auth)/login/page.tsx` |
| `login_failed` | Sign-in attempt failed, with `reason` property | `src/app/(auth)/login/page.tsx` |
| `case_opened` | Analyst opens a case investigation (existing, wired in client) | `src/components/workbench/case-investigation-client.tsx` |
| `verdict_approved` | Analyst approves the AI triage verdict (existing) | `src/components/workbench/case-workbench.tsx` |
| `verdict_overridden` | Analyst overrides the AI triage verdict (existing) | `src/components/workbench/case-workbench.tsx` |
| `command_palette_used` | Analyst navigates via the command palette (existing) | `src/components/workbench/command-palette.tsx` |
| `simulation_run` | Single simulation run with `scenario_id`, `verdict`, `execution_path`, `latency_ms`, `escalated` | `src/app/workbench/simulate/page.tsx` |
| `batch_simulation_run` | 50-concurrent batch run with `scenario_id`, `n`, `p95_ms`, `p99_ms`, `errors` | `src/app/workbench/simulate/page.tsx` |
| `hunt_query_run` | Analyst executes a ClickHouse SQL query against the warm tier | `src/app/workbench/hunt/page.tsx` |

## Next steps

We've built some insights and a dashboard for you to keep an eye on user behavior, based on the events we just instrumented:

- **Dashboard — Analytics basics**: https://us.posthog.com/project/405558/dashboard/1533041
- **Login funnel: Sign-in success vs failure**: https://us.posthog.com/project/405558/insights/H6RowOr8
- **Verdict decisions over time**: https://us.posthog.com/project/405558/insights/BzeKRx8i
- **Case investigation activity**: https://us.posthog.com/project/405558/insights/H6NHUvH8
- **Simulation lab usage**: https://us.posthog.com/project/405558/insights/DkhkMc58
- **Power feature usage: Command palette & Hunt queries**: https://us.posthog.com/project/405558/insights/W7FhEj7s

### Agent skill

We've left an agent skill folder in your project. You can use this context for further agent development when using Claude Code. This will help ensure the model provides the most up-to-date approaches for integrating PostHog.

</wizard-report>
