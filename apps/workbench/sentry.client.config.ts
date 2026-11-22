import * as Sentry from "@sentry/nextjs";
import { shouldInitSentry } from "@/lib/telemetry";

if (shouldInitSentry()) Sentry.init({
  dsn: process.env.NEXT_PUBLIC_SENTRY_DSN,
  enabled: true,
  tracesSampleRate: 0.05,
  replaysSessionSampleRate: 0,
  replaysOnErrorSampleRate: 0,
});
