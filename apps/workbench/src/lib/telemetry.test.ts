import { describe, expect, test } from "bun:test";
import { shouldInitPosthog, shouldInitSentry, telemetryEnabled } from "./telemetry";

const key = "phc_test";
const dsn = "https://public@o0.ingest.sentry.io/0";

describe("telemetry init", () => {
  test("stays off when the flag is missing, even if keys are set", () => {
    const env = { NEXT_PUBLIC_POSTHOG_KEY: key, NEXT_PUBLIC_SENTRY_DSN: dsn };
    expect(telemetryEnabled(env)).toBe(false);
    expect(shouldInitPosthog(env)).toBe(false);
    expect(shouldInitSentry(env)).toBe(false);
  });

  test("stays off when the flag is on but the vendor key is missing", () => {
    const env = { ATTEST_TELEMETRY: "on", NEXT_PUBLIC_ATTEST_TELEMETRY: "on" };
    expect(telemetryEnabled(env)).toBe(true);
    expect(shouldInitPosthog(env)).toBe(false);
    expect(shouldInitSentry(env)).toBe(false);
  });

  test("initializes only when the flag is on and that vendor's key is set", () => {
    expect(shouldInitPosthog({ NEXT_PUBLIC_ATTEST_TELEMETRY: "on", NEXT_PUBLIC_POSTHOG_KEY: key })).toBe(
      true,
    );
    expect(shouldInitSentry({ ATTEST_TELEMETRY: "on", SENTRY_DSN: dsn })).toBe(true);
    expect(shouldInitPosthog({ ATTEST_TELEMETRY: "off", NEXT_PUBLIC_POSTHOG_KEY: key })).toBe(false);
  });
});
