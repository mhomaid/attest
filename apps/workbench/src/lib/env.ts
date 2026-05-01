import { z } from "zod";

const clientEnvSchema = z.object({
  NEXT_PUBLIC_APP_URL: z.string().url().optional(),
  NEXT_PUBLIC_CP_URL: z
    .string()
    .url()
    .default("http://localhost:8080"),
  NEXT_PUBLIC_CP_WS_URL: z.string().min(1).default("ws://localhost:8080"),
  NEXT_PUBLIC_WS_GATEWAY_URL: z
    .string()
    .min(1)
    .default("ws://localhost:4500"),
  NEXT_PUBLIC_POSTHOG_KEY: z.string().optional(),
  NEXT_PUBLIC_POSTHOG_HOST: z.string().url().optional(),
  NEXT_PUBLIC_SENTRY_DSN: z.string().url().optional(),
});

/** Validated public env (safe for client bundles). */
export const env = clientEnvSchema.parse({
  NEXT_PUBLIC_APP_URL: process.env.NEXT_PUBLIC_APP_URL,
  NEXT_PUBLIC_CP_URL:
    process.env.NEXT_PUBLIC_CP_URL ?? process.env.NEXT_PUBLIC_API_URL,
  NEXT_PUBLIC_CP_WS_URL:
    process.env.NEXT_PUBLIC_CP_WS_URL ?? process.env.NEXT_PUBLIC_WS_URL,
  NEXT_PUBLIC_WS_GATEWAY_URL: process.env.NEXT_PUBLIC_WS_GATEWAY_URL,
  NEXT_PUBLIC_POSTHOG_KEY: process.env.NEXT_PUBLIC_POSTHOG_KEY,
  NEXT_PUBLIC_POSTHOG_HOST: process.env.NEXT_PUBLIC_POSTHOG_HOST,
  NEXT_PUBLIC_SENTRY_DSN: process.env.NEXT_PUBLIC_SENTRY_DSN,
});
