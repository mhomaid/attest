import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";
import { ROLES } from "@/lib/roles";

let session: { user: { id: string; email: string; role?: string } } | null = null;

mock.module("next/headers", () => ({ headers: async () => new Headers() }));
mock.module("@/lib/auth", () => ({
  auth: { api: { getSession: async () => session } },
}));

const { POST } = await import("./route");

const realFetch = globalThis.fetch;
let upstreamCalls = 0;

beforeEach(() => {
  session = null;
  upstreamCalls = 0;
  globalThis.fetch = (async () => {
    upstreamCalls += 1;
    return new Response(JSON.stringify({ agent_action_id: "a1" }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  }) as unknown as typeof fetch;
});

afterEach(() => {
  globalThis.fetch = realFetch;
});

function call() {
  return POST(
    new Request("http://localhost/api/cases/c1/override", {
      method: "POST",
      body: JSON.stringify({ verdict: "benign", reason: "unit test" }),
    }),
    { params: Promise.resolve({ caseId: "c1" }) },
  );
}

describe("POST /api/cases/:id/override", () => {
  test("anonymous → 401 and nothing is forwarded", async () => {
    const res = await call();
    expect(res.status).toBe(401);
    expect(upstreamCalls).toBe(0);
  });

  test("viewer cannot perform an analyst action → 403 and nothing is forwarded", async () => {
    session = { user: { id: "u1", email: "v@example.com", role: "viewer" } };
    const res = await call();
    expect(res.status).toBe(403);
    expect(upstreamCalls).toBe(0);
  });

  test("auditor cannot override → 403", async () => {
    session = { user: { id: "u3", email: "au@example.com", role: "auditor" } };
    const res = await call();
    expect(res.status).toBe(403);
    expect(upstreamCalls).toBe(0);
  });

  for (const role of ["analyst", "lead"]) {
    test(`${role} may override → forwarded`, async () => {
      session = { user: { id: "u2", email: `${role}@example.com`, role } };
      const res = await call();
      expect(res.status).toBe(200);
      expect(upstreamCalls).toBe(1);
    });
  }

  test("every role is covered by this file", () => {
    expect([...ROLES].sort()).toEqual(["analyst", "auditor", "lead", "viewer"]);
  });
});
