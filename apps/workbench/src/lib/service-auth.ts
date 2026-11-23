import { createHmac } from "node:crypto";
import jwt from "jsonwebtoken";

/** Local compose defaults. Production must set the real secrets. */
export const DEV_SERVICE_AUTH_SECRET = "dev-only-service-auth-secret";
export const DEV_INGEST_HMAC_SECRET = "dev-only-ingest-hmac-secret";

export const AUDIENCE = {
  controlPlane: "control-plane",
  orchestrator: "orchestrator",
  mcpGateway: "mcp-gateway",
  wsGateway: "ws-gateway",
} as const;

export type AuthEnv = {
  NODE_ENV?: string;
  ATTEST_SERVICE_AUTH_SECRET?: string;
  ATTEST_INGEST_HMAC_SECRET?: string;
};

export function serviceAuthSecret(env: AuthEnv = process.env): string {
  const configured = env.ATTEST_SERVICE_AUTH_SECRET;
  if (configured && configured.length > 0) return configured;
  if (env.NODE_ENV === "production") return "";
  return DEV_SERVICE_AUTH_SECRET;
}

/** HS256 service token. Undefined when production has no secret configured. */
export function serviceAuthorization(
  audience: string,
  env: AuthEnv = process.env,
): string | undefined {
  const secret = serviceAuthSecret(env);
  if (!secret) return undefined;
  const token = jwt.sign({ aud: audience }, secret, {
    algorithm: "HS256",
    expiresIn: 120,
  });
  return `Bearer ${token}`;
}

export function ingestHmacSecret(env: AuthEnv = process.env): string {
  const configured = env.ATTEST_INGEST_HMAC_SECRET;
  if (configured && configured.length > 0) return configured;
  if (env.NODE_ENV === "production") return "";
  return DEV_INGEST_HMAC_SECRET;
}

/** Sign the exact body bytes the collector will verify. */
export function withIngestHmac(body: string, init: RequestInit = {}): RequestInit {
  const secret = ingestHmacSecret();
  const headers = new Headers(init.headers);
  if (secret) {
    const timestamp = Math.floor(Date.now() / 1000).toString();
    const signature = createHmac("sha256", secret)
      .update(timestamp)
      .update(".")
      .update(body)
      .digest("hex");
    headers.set("x-attest-timestamp", timestamp);
    headers.set("x-attest-signature", signature);
  }
  return { ...init, headers, body };
}

export function withServiceAuth(audience: string, init: RequestInit = {}): RequestInit {
  const authorization = serviceAuthorization(audience);
  if (!authorization) return init;
  const headers = new Headers(init.headers);
  headers.set("authorization", authorization);
  return { ...init, headers };
}
