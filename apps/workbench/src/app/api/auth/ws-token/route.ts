import { requirePermission } from "@/lib/authz";
import { SignJWT } from "jose";
import { NextResponse } from "next/server";

const DEFAULT_TENANT = "default";

function getWsSecret(): Uint8Array {
  const s = process.env.ATTEST_WS_JWT_SECRET ?? "dev-ws-jwt-secret-change-me-min-32-chars!";
  return new TextEncoder().encode(s);
}

/** POST { case_id: string } — short-lived HS256 JWT for trace WebSocket upgrade. */
export async function POST(req: Request) {
  const authz = await requirePermission("case.view");
  if (!authz.ok) return authz.response;

  let body: { case_id?: string };
  try {
    body = (await req.json()) as { case_id?: string };
  } catch {
    return NextResponse.json({ error: "invalid JSON" }, { status: 400 });
  }

  const case_id = body.case_id?.trim();
  if (!case_id) {
    return NextResponse.json({ error: "case_id required" }, { status: 400 });
  }

  const token = await new SignJWT({
    sub: authz.user.id,
    email: authz.user.email,
    tenant_id: DEFAULT_TENANT,
    case_id,
  })
    .setProtectedHeader({ alg: "HS256" })
    .setIssuedAt()
    .setExpirationTime("60s")
    .sign(getWsSecret());

  return NextResponse.json({ token, expires_in: 60 });
}
