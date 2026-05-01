/**
 * PostHog singleton re-export.
 *
 * Initialisation is handled by instrumentation-client.ts (Next.js 15.3+ pattern).
 *
 * PII rules (from 12_Workbench.md §2.3):
 * - Session replay masks all text inputs and [data-customer-data] elements.
 * - Customer identifiers are only sent as hashed values — never in plaintext.
 * - No customer alert content, case content, or reasoning traces are
 *   ever sent to PostHog.
 */

import posthog from "posthog-js";

export { posthog };
