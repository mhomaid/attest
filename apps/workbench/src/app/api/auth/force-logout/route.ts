import { NextResponse } from "next/server";

const SESSION_COOKIES = [
  "better-auth.session_token",
  "__Secure-better-auth.session_token",
  "better-auth.session_data",
  "__Secure-better-auth.session_data",
];

function clearSessionCookies(res: NextResponse) {
  for (const name of SESSION_COOKIES) {
    res.cookies.set(name, "", {
      path: "/",
      maxAge: 0,
      sameSite: "lax",
      httpOnly: true,
      secure: name.startsWith("__Secure-"),
    });
  }
}

export async function GET(req: Request) {
  const res = NextResponse.redirect(new URL("/login", req.url));
  clearSessionCookies(res);
  return res;
}

export async function POST() {
  const res = NextResponse.json({ ok: true });
  clearSessionCookies(res);

  return res;
}
