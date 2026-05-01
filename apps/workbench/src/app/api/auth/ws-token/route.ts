import { auth } from "@/lib/auth";
import { SignJWT } from "jose";
import { headers } from "next/headers";
import { NextResponse } from "next/server";

const DEFAULT_TENANT = "default";

function getWsSecret(): Uint8Array {
  const s = process.env.ATTEST_WS_JWT_SECRET ?? "dev-ws-jwt-secret-change-me-min-32-chars!";
  return new TextEncoder().encode(s);
}

/** POST { case_id: string } — short-lived HS256 JWT for trace WebSocket upgrade. */
export async function POST(req: Request) {
  const hdrs = await headers();
  const session = await auth.api.getSession({ headers: hdrs });
  if (!session?.user) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

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

  const email = session.user.email ?? "";
  const token = await new SignJWT({
    sub: session.user.id,
    email,
    tenant_id: DEFAULT_TENANT,
    case_id,
  })
    .setProtectedHeader({ alg: "HS256" })
    .setIssuedAt()
    .setExpirationTime("60s")
    .sign(getWsSecret());

  return NextResponse.json({ token, expires_in: 60 });
}
