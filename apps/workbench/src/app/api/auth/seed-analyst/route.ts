import { auth } from "@/lib/auth";
import { db } from "@/lib/db";
import { NextResponse } from "next/server";

const SEED_EMAIL = "analyst@attest.local";

/**
 * Optional dev fallback: creates analyst@attest.local via Better Auth sign-up.
 * Prefer `infra/db` Alembic migration `0002_seed_dev_analyst` (runs with `alembic upgrade head`).
 */
export async function POST(req: Request) {
  if (process.env.ALLOW_AUTH_SEED !== "1") {
    return NextResponse.json({ error: "not found" }, { status: 404 });
  }

  const secret = req.headers.get("x-seed-secret");
  if (!secret || secret !== process.env.AUTH_SEED_SECRET) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const existing = await db
    .selectFrom("auth_users")
    .select("id")
    .where("email", "=", SEED_EMAIL)
    .executeTakeFirst();

  if (existing) {
    return NextResponse.json({
      ok: true,
      created: false,
      email: SEED_EMAIL,
    });
  }

  const url = new URL(req.url);
  const signUp = new Request(`${url.origin}/api/auth/sign-up/email`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      name: "Analyst",
      email: SEED_EMAIL,
      password: "analyst-dev",
    }),
  });

  return auth.handler(signUp);
}
