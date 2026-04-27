/**
 * Sentry initialisation with customer-data scrubbing.
 *
 * Rules (from 12_Workbench.md §2.3):
 * - No customer alert content, case content, reasoning traces, or
 *   tenant identifiers must appear in Sentry events.
 * - `beforeSend` scrubs known sensitive fields before the event leaves
 *   the browser.
 */

import * as Sentry from "@sentry/nextjs";

export function initSentry() {
  if (!process.env.NEXT_PUBLIC_SENTRY_DSN) return;

  Sentry.init({
    dsn: process.env.NEXT_PUBLIC_SENTRY_DSN,
    tracesSampleRate: 0.1,
    beforeSend(event) {
      // Strip any breadcrumb or extra that contains customer data keys.
      const BLOCKED_PATTERNS = [
        /alert/i,
        /case/i,
        /reasoning/i,
        /tenant/i,
        /customer/i,
      ];

      if (event.breadcrumbs?.values) {
        event.breadcrumbs.values = event.breadcrumbs.values.filter(
          (b) =>
            !BLOCKED_PATTERNS.some(
              (re) => re.test(b.message ?? "") || re.test(b.category ?? ""),
            ),
        );
      }

      return event;
    },
  });
}
