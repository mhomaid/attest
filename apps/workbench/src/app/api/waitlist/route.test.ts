import { afterEach, describe, expect, test } from "bun:test";
import { NextRequest } from "next/server";
import { POST } from "./route";

const saved = process.env.ATTEST_AUTH_PASSWORD;
afterEach(() => {
  if (saved === undefined) delete process.env.ATTEST_AUTH_PASSWORD;
  else process.env.ATTEST_AUTH_PASSWORD = saved;
});

function req() {
  return new NextRequest("http://localhost/api/waitlist", {
    method: "POST",
    body: JSON.stringify({}),
    headers: { "x-forwarded-for": "203.0.113.9" },
  });
}

describe("POST /api/waitlist", () => {
  test("ATTEST_AUTH_PASSWORD=off disables the waitlist (404)", async () => {
    process.env.ATTEST_AUTH_PASSWORD = "off";
    const res = await POST(req());
    expect(res.status).toBe(404);
  });

  test("still validates input when password auth is on", async () => {
    delete process.env.ATTEST_AUTH_PASSWORD;
    const res = await POST(req());
    expect(res.status).toBe(400);
  });
});
