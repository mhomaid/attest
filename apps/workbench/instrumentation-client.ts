import posthog from "posthog-js";

posthog.init(process.env.NEXT_PUBLIC_POSTHOG_KEY!, {
  api_host: "/ingest",
  ui_host: "https://us.posthog.com",
  defaults: "2026-01-30",
  capture_pageview: false,
  capture_exceptions: true,
  session_recording: {
    maskAllInputs: true,
    maskTextSelector: "[data-customer-data]",
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
