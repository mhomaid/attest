import { NextRequest, NextResponse } from "next/server";
import { formatWaitlistEmail, waitlistSchema } from "@/lib/waitlist";

const WINDOW_MS = 60 * 60 * 1000;
const PER_IP_LIMIT = 3;
const GLOBAL_LIMIT = 30;
const hits = new Map<string, number[]>();

function rateLimited(ip: string): boolean {
  const now = Date.now();
  const recent = (key: string) => (hits.get(key) ?? []).filter((t) => now - t < WINDOW_MS);
  const mine = recent(ip);
  const all = recent("*");
  if (mine.length >= PER_IP_LIMIT || all.length >= GLOBAL_LIMIT) return true;
  hits.set(ip, [...mine, now]);
  hits.set("*", [...all, now]);
  if (hits.size > 10_000) hits.clear();
  return false;
}

export async function POST(req: NextRequest) {
  const ip = req.headers.get("x-forwarded-for")?.split(",")[0]?.trim() || "unknown";
  if (rateLimited(ip)) {
    return NextResponse.json(
      { ok: false, error: "Too many requests. Try again in an hour." },
      { status: 429 },
    );
  }

  let body: unknown;
  try {
    body = await req.json();
  } catch {
    return NextResponse.json({ ok: false, error: "Invalid JSON" }, { status: 400 });
  }

  const parsed = waitlistSchema.safeParse(body);
  if (!parsed.success) {
    return NextResponse.json(
      { ok: false, error: "Check the form and try again." },
      { status: 400 },
    );
  }

  // Bots that fill the hidden field are dropped silently.
  if (parsed.data.website) {
    return NextResponse.json({ ok: true });
  }

  const apiKey = process.env.RESEND_API_KEY?.trim();
  const from =
    process.env.RESEND_FROM?.trim() || "Attest Waitlist <admin@homaid.dev>";
  const to = process.env.WAITLIST_NOTIFY_TO?.trim() || "mohamed@homaid.dev";
  const { subject, text } = formatWaitlistEmail(parsed.data, ip);

  if (!apiKey) {
    if (process.env.NODE_ENV === "production") {
      return NextResponse.json(
        { ok: false, error: "Waitlist email is not configured yet." },
        { status: 503 },
      );
    }
    console.info("[waitlist] RESEND_API_KEY unset — local accept\n", text);
    return NextResponse.json({ ok: true, delivered: false });
  }

  const payload = {
    to: [to],
    reply_to: parsed.data.email,
    subject,
    text,
  };

  let res = await fetch("https://api.resend.com/emails", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ from, ...payload }),
  });

  // Domain verify is pending until send.* CNAMEs land. Resend still delivers
  // to the account inbox from onboarding@resend.dev in the meantime.
  if (!res.ok && from !== "Attest Waitlist <onboarding@resend.dev>") {
    const detail = await res.text();
    if (detail.includes("domain is not verified") || detail.includes("not verified")) {
      res = await fetch("https://api.resend.com/emails", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${apiKey}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          from: "Attest Waitlist <onboarding@resend.dev>",
          ...payload,
        }),
      });
    } else {
      console.error("[waitlist] resend failed", res.status, detail.slice(0, 400));
      return NextResponse.json(
        { ok: false, error: "Could not send the request. Try again shortly." },
        { status: 502 },
      );
    }
  }

  if (!res.ok) {
    const detail = await res.text();
    console.error("[waitlist] resend failed", res.status, detail.slice(0, 400));
    return NextResponse.json(
      { ok: false, error: "Could not send the request. Try again shortly." },
      { status: 502 },
    );
  }

  return NextResponse.json({ ok: true, delivered: true });
}
