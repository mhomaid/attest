import { NextResponse } from "next/server";

const CP_URL = process.env.CONTROL_PLANE_URL ?? "http://localhost:8080";

/**
 * GET /api/detections
 * Proxies to the control-plane's /v1/detections/fired endpoint.
 * Returns the flat array of FiredDetection objects.
 */
export async function GET() {
  try {
    const res = await fetch(`${CP_URL}/v1/detections/fired`, {
      // Never cache — we always want the freshest snapshot for the server render.
      cache: "no-store",
    });

    if (!res.ok) {
      return NextResponse.json(
        { error: `control-plane error: ${res.status}` },
        { status: res.status },
      );
    }

    const data = await res.json();
    return NextResponse.json(data);
  } catch {
    return NextResponse.json(
      { error: "control-plane unreachable" },
      { status: 503 },
    );
  }
}
