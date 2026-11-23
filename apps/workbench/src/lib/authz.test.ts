import { beforeEach, describe, expect, mock, test } from "bun:test";

let session: { user: { id: string; email: string; role?: string } } | null = null;

mock.module("next/headers", () => ({ headers: async () => new Headers() }));
mock.module("@/lib/auth", () => ({
  auth: { api: { getSession: async () => session } },
}));

const { requirePermission } = await import("./authz");

beforeEach(() => {
  session = null;
});

describe("requirePermission", () => {
  test("no session → 401", async () => {
    const r = await requirePermission("case.view");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.response.status).toBe(401);
  });

  test("viewer asking for an analyst action → 403", async () => {
    session = { user: { id: "u1", email: "v@example.com", role: "viewer" } };
    const r = await requirePermission("case.override");
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.response.status).toBe(403);
      expect(await r.response.json()).toEqual({ error: "forbidden" });
    }
  });

  test("a user with no stored role is treated as viewer", async () => {
    session = { user: { id: "u1", email: "v@example.com" } };
    const r = await requirePermission("case.override");
    expect(r.ok).toBe(false);
  });

  test("a user with a garbage stored role gets nothing", async () => {
    session = { user: { id: "u1", email: "v@example.com", role: "superadmin" } };
    expect((await requirePermission("case.view")).ok).toBe(false);
  });

  test("analyst passes case.override and the result carries user and role", async () => {
    session = { user: { id: "u2", email: "a@example.com", role: "analyst" } };
    const r = await requirePermission("case.override");
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.role).toBe("analyst");
      expect(r.user.id).toBe("u2");
    }
  });
});
