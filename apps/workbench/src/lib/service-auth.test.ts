import { describe, expect, test } from "bun:test";
import jwt from "jsonwebtoken";
import { AUDIENCE, serviceAuthorization, serviceAuthSecret } from "./service-auth";

describe("service auth", () => {
  test("production without a secret does not invent a token", () => {
    const env = { NODE_ENV: "production" };
    expect(serviceAuthSecret(env)).toBe("");
    expect(serviceAuthorization(AUDIENCE.controlPlane, env)).toBeUndefined();
  });

  test("a configured secret produces an HS256 token for that audience", () => {
    const env = { ATTEST_SERVICE_AUTH_SECRET: "unit-test-secret" };
    const header = serviceAuthorization(AUDIENCE.orchestrator, env);
    expect(header?.startsWith("Bearer ")).toBe(true);
    const decoded = jwt.verify(header!.slice("Bearer ".length), "unit-test-secret", {
      algorithms: ["HS256"],
      audience: AUDIENCE.orchestrator,
    }) as { aud: string };
    expect(decoded.aud).toBe(AUDIENCE.orchestrator);
  });
});
