import { auth } from "@/lib/auth";
import { AUDIENCE, serviceAuthorization } from "@/lib/service-auth";
import { headers } from "next/headers";
import { NextResponse } from "next/server";

const ALLOWED = new Set<string>(Object.values(AUDIENCE));

/**
 * GET /api/auth/service-token?audience=control-plane
 * Short-lived service token for a browser WebSocket. Requires a signed-in session.
 */
export async function GET(req: Request) {
  const hdrs = await headers();
  const session = await auth.api.getSession({ headers: hdrs });
  if (!session?.user) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const audience = new URL(req.url).searchParams.get("audience") ?? "";
  if (!ALLOWED.has(audience)) {
    return NextResponse.json({ error: "unknown audience" }, { status: 400 });
  }

  const authorization = serviceAuthorization(audience);
  if (!authorization) {
    return NextResponse.json({ error: "service auth is not configured" }, { status: 503 });
  }

  return NextResponse.json({
    token: authorization.slice("Bearer ".length),
    expires_in: 120,
  });
}
