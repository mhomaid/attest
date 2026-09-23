"use client";

import { Loader2, Shield } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { authClient } from "@/lib/auth-client";

export function LoginForm({
  callbackUrl,
  showLocalHints,
}: {
  callbackUrl: string;
  showLocalHints: boolean;
}) {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const next = callbackUrl.startsWith("/") ? callbackUrl : "/workbench/queue";

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const res = await authClient.signIn.email({ email, password });
      if (res.error) {
        setError(res.error.message ?? "Sign-in failed");
        return;
      }
      router.push(next);
      router.refresh();
    } catch {
      setError("Sign-in failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="relative flex min-h-dvh flex-col items-center justify-center px-4 py-16">
      <div className="w-full max-w-sm">
        <Link
          href="/"
          className="mb-8 flex items-center justify-center gap-2 font-semibold tracking-tight"
        >
          <span className="grid h-8 w-8 place-items-center rounded-md border border-primary/35 bg-primary/10 text-primary">
            <Shield className="h-4 w-4" />
          </span>
          Attest
        </Link>

        <div className="rounded-xl border border-border/70 bg-card/50 p-6 backdrop-blur-sm">
          <h1 className="text-lg font-semibold tracking-tight">Sign in</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Invite-only. Request access if you do not have an account yet.
          </p>

          <form onSubmit={onSubmit} className="mt-6 space-y-4">
            <div className="space-y-1.5">
              <label htmlFor="email" className="text-xs text-muted-foreground">
                Email
              </label>
              <input
                id="email"
                type="email"
                autoComplete="username"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
                className="h-9 w-full rounded-md border border-border bg-background/80 px-3 text-sm outline-none focus:border-primary/50"
              />
            </div>

            <div className="space-y-1.5">
              <label htmlFor="password" className="text-xs text-muted-foreground">
                Password
              </label>
              <input
                id="password"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                minLength={8}
                className="h-9 w-full rounded-md border border-border bg-background/80 px-3 text-sm outline-none focus:border-primary/50"
              />
            </div>

            {error ? (
              <p className="text-xs text-destructive" role="alert">
                {error}
              </p>
            ) : null}

            <button
              type="submit"
              disabled={busy}
              className="inline-flex h-9 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm font-medium text-primary-foreground disabled:opacity-60"
            >
              {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
              Sign in
            </button>
          </form>
        </div>

        <div className="mt-6 space-y-2 text-center text-xs text-muted-foreground">
          <p>
            <Link href="/waitlist" className="underline underline-offset-4 hover:text-foreground">
              Request access
            </Link>
            {" · "}
            <Link href="/#try" className="underline underline-offset-4 hover:text-foreground">
              Run a verdict without signing in
            </Link>
          </p>
          {showLocalHints ? (
            <>
              <p>
                Public demo:{" "}
                <code className="text-foreground">demo@attest.local</code> /{" "}
                <code className="text-foreground">try-attest</code>
              </p>
              <p>
                Local analyst:{" "}
                <code className="text-foreground">analyst@attest.local</code> /{" "}
                <code className="text-foreground">analyst-dev</code>
              </p>
            </>
          ) : null}
        </div>
      </div>
    </div>
  );
}
