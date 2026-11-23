import { beforeEach, describe, expect, mock, test } from "bun:test";
import { labelsToCsv, type AnalystLabel } from "@/lib/labels";
import { ROLES } from "@/lib/roles";

let session: { user: { id: string; email: string; role?: string } } | null = null;
const saved: AnalystLabel[] = [];
let listed = 0;

mock.module("next/headers", () => ({ headers: async () => new Headers() }));
mock.module("@/lib/auth", () => ({
  auth: { api: { getSession: async () => session } },
}));
mock.module("@/lib/server/label-repository", () => ({
  saveAnalystLabel: async (row: AnalystLabel) => {
    saved.push(row);
    return row;
  },
  listAnalystLabels: async () => {
    listed += 1;
    return [...saved];
  },
}));

const { POST } = await import("./route");
const { GET } = await import("../../../labels/export/route");

const ENVELOPE = "11111111-1111-4111-8111-111111111111";
const CASE_ID = "33333333-3333-4333-8333-333333333333";

beforeEach(() => {
  session = null;
  saved.length = 0;
  listed = 0;
});

function post(body: unknown, caseId = CASE_ID) {
  return POST(
    new Request(`http://localhost/api/cases/${caseId}/labels`, {
      method: "POST",
      body: JSON.stringify(body),
    }),
    { params: Promise.resolve({ caseId }) },
  );
}

function exported(format?: string) {
  const query = format === undefined ? "" : `?format=${encodeURIComponent(format)}`;
  return GET(new Request(`http://localhost/api/labels/export${query}`));
}

const valid = {
  envelope_id: ENVELOPE,
  agreement: "disagree",
  disposition: "benign",
  reason: "legitimate travel for tenant-a",
  user_id: "not-the-session",
  created_at: "1999-01-01T00:00:00.000Z",
};

describe("POST /api/cases/:id/labels", () => {
  test("anonymous → 401 and nothing is stored", async () => {
    const res = await post(valid);
    expect(res.status).toBe(401);
    expect(saved).toHaveLength(0);
  });

  test("viewer cannot label → 403 and nothing is stored", async () => {
    session = { user: { id: "user-viewer", email: "viewer@example.com", role: "viewer" } };
    const res = await post(valid);
    expect(res.status).toBe(403);
    expect(await res.json()).toEqual({ error: "forbidden" });
    expect(saved).toHaveLength(0);
  });

  test("auditor cannot label → 403 and nothing is stored", async () => {
    session = { user: { id: "user-auditor", email: "auditor@example.com", role: "auditor" } };
    const res = await post(valid);
    expect(res.status).toBe(403);
    expect(saved).toHaveLength(0);
  });

  for (const role of ["analyst", "lead"] as const) {
    test(`${role} records agreement, disposition and reason with user, time and envelope id`, async () => {
      const before = Date.now();
      session = { user: { id: "user-1001", email: `${role}@example.com`, role } };
      const res = await post({ ...valid, agreement: role === "lead" ? "agree" : "disagree" });
      expect(res.status).toBe(201);
      const row = saved[0];
      expect(row).toBeDefined();
      const body = await res.json();
      expect(body).toEqual(row);
      expect(row?.user_id).toBe("user-1001");
      expect(row?.user_email).toBe(`${role}@example.com`);
      expect(row?.envelope_id).toBe(ENVELOPE);
      expect(row?.case_id).toBe(CASE_ID);
      expect(row?.agreement).toBe(role === "lead" ? "agree" : "disagree");
      expect(row?.disposition).toBe("benign");
      expect(row?.reason).toBe("legitimate travel for tenant-a");
      expect(row?.created_at).not.toBe("1999-01-01T00:00:00.000Z");
      const storedAt = Date.parse(row?.created_at ?? "");
      expect(storedAt).toBeGreaterThanOrEqual(before);
      expect(storedAt).toBeLessThanOrEqual(Date.now() + 1000);
    });
  }

  test("analyst with an invalid body does not store a label", async () => {
    session = { user: { id: "user-1001", email: "analyst@example.com", role: "analyst" } };
    const res = await post({ ...valid, reason: "   " });
    expect(res.status).toBe(400);
    expect(saved).toHaveLength(0);
  });

  test("every role is covered by this file", () => {
    expect([...ROLES].sort()).toEqual(["analyst", "auditor", "lead", "viewer"]);
  });
});

describe("GET /api/labels/export", () => {
  test("anonymous → 401 and the store is not read", async () => {
    const res = await exported();
    expect(res.status).toBe(401);
    expect(listed).toBe(0);
  });

  test("viewer cannot export → 403", async () => {
    session = { user: { id: "user-viewer", email: "viewer@example.com", role: "viewer" } };
    const res = await exported();
    expect(res.status).toBe(403);
    expect(listed).toBe(0);
  });

  test("auditor cannot export → 403", async () => {
    session = { user: { id: "user-auditor", email: "auditor@example.com", role: "auditor" } };
    const res = await exported();
    expect(res.status).toBe(403);
    expect(listed).toBe(0);
  });

  for (const role of ["analyst", "lead"] as const) {
    test(`${role} exports the stored label to csv with user, time and envelope id`, async () => {
      session = { user: { id: "user-1001", email: `${role}@example.com`, role } };
      const created = await post({
        ...valid,
        agreement: "disagree",
        reason: 'travel for tenant-a, analyst said "no"',
      });
      expect(created.status).toBe(201);
      const res = await exported("csv");
      expect(res.status).toBe(200);
      expect(res.headers.get("content-type")).toContain("text/csv");
      expect(res.headers.get("content-disposition")).toContain("analyst-labels.csv");
      expect(await res.text()).toBe(labelsToCsv(saved));
      expect(listed).toBe(1);
      expect(saved[0]?.envelope_id).toBe(ENVELOPE);
      expect(saved[0]?.user_id).toBe("user-1001");
    });
  }

  test("parquet is not produced", async () => {
    session = { user: { id: "user-1001", email: "analyst@example.com", role: "analyst" } };
    const res = await exported("parquet");
    expect(res.status).toBe(400);
    expect(await res.json()).toEqual({ error: "unsupported format" });
    expect(listed).toBe(0);
  });
});
