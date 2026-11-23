import { headers } from "next/headers";
import { NextResponse } from "next/server";
import { auth } from "@/lib/auth";
import { can, PERMISSIONS, parseRole } from "@/lib/roles";

/** GET /api/auth/me — the signed-in user's id, email and role. 401 without a session. */
export async function GET() {
  const session = await auth.api.getSession({ headers: await headers() });
  if (!session?.user) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const stored = (session.user as { role?: unknown }).role;
  const role = stored == null ? "viewer" : parseRole(stored);
  return NextResponse.json({
    id: session.user.id,
    email: session.user.email,
    role: role ?? null,
    permissions: PERMISSIONS.filter((p) => can(role, p)),
  });
}
