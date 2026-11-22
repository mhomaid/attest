import { headers } from "next/headers";
import { NextResponse } from "next/server";
import { auth } from "@/lib/auth";
import { can, parseRole, type Permission, type Role } from "@/lib/roles";

export type AuthzResult =
  | { ok: true; role: Role; user: { id: string; email: string } }
  | { ok: false; response: NextResponse };

/**
 * Server-side gate for route handlers. 401 without a session, 403 when the session's role
 * lacks the permission. A user with no stored role is a viewer; an unrecognised stored role
 * gets nothing.
 */
export async function requirePermission(permission: Permission): Promise<AuthzResult> {
  const session = await auth.api.getSession({ headers: await headers() });
  if (!session?.user) {
    return { ok: false, response: NextResponse.json({ error: "unauthorized" }, { status: 401 }) };
  }
  const stored = (session.user as { role?: unknown }).role;
  const role = stored == null ? "viewer" : parseRole(stored);
  if (!role || !can(role, permission)) {
    return { ok: false, response: NextResponse.json({ error: "forbidden" }, { status: 403 }) };
  }
  return { ok: true, role, user: { id: session.user.id, email: session.user.email ?? "" } };
}
