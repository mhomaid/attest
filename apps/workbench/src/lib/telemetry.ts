/**
 * Telemetry is off unless an operator turns it on and supplies a vendor key.
 * Browser code only sees NEXT_PUBLIC_* variables. Server code sees both.
 */

export type TelemetryEnv = {
  ATTEST_TELEMETRY?: string;
  NEXT_PUBLIC_ATTEST_TELEMETRY?: string;
  NEXT_PUBLIC_POSTHOG_KEY?: string;
  NEXT_PUBLIC_SENTRY_DSN?: string;
  SENTRY_DSN?: string;
};

function readEnv(env?: TelemetryEnv): TelemetryEnv {
  return (env ?? process.env) as TelemetryEnv;
}

export function telemetryEnabled(env?: TelemetryEnv): boolean {
  const bag = readEnv(env);
  return bag.ATTEST_TELEMETRY === "on" || bag.NEXT_PUBLIC_ATTEST_TELEMETRY === "on";
}

export function shouldInitPosthog(env?: TelemetryEnv): boolean {
  const bag = readEnv(env);
  return telemetryEnabled(bag) && Boolean(bag.NEXT_PUBLIC_POSTHOG_KEY);
}

export function shouldInitSentry(env?: TelemetryEnv): boolean {
  const bag = readEnv(env);
  const dsn = bag.NEXT_PUBLIC_SENTRY_DSN || bag.SENTRY_DSN;
  return telemetryEnabled(bag) && Boolean(dsn);
}
