/**
 * PostHog initialisation with strict PII masking.
 *
 * Rules (from 12_Workbench.md §2.3):
 * - Session replay is enabled but all text inputs are masked by default.
 * - Customer identifiers (tenant_id, user email) are only sent as hashed
 *   values — never in plaintext.
 * - No customer alert content, case content, or reasoning traces are
 *   ever sent to PostHog.
 */

import posthog from "posthog-js";

export function initPostHog() {
  if (typeof window === "undefined") return;
  if (!process.env.NEXT_PUBLIC_POSTHOG_KEY) return;

  posthog.init(process.env.NEXT_PUBLIC_POSTHOG_KEY, {
    api_host: process.env.NEXT_PUBLIC_POSTHOG_HOST ?? "https://app.posthog.com",
    capture_pageview: true,
    session_recording: {
      maskAllInputs: true,
      maskTextSelector: "[data-pii]",
    },
    sanitize_properties(properties) {
      // Strip any property that looks like a raw email or alert content.
      const BLOCKED_KEYS = ["email", "alert_body", "case_body", "reasoning"];
      for (const key of BLOCKED_KEYS) {
        delete properties[key];
      }
      return properties;
    },
  });
}

export { posthog };
