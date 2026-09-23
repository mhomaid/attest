"use client";

import { Loader2, Shield } from "lucide-react";
import Link from "next/link";
import { useState } from "react";
import { analytics } from "@/lib/analytics";
import { ROLE_LABELS, WAITLIST_ROLES } from "@/lib/waitlist";

type Status = "idle" | "busy" | "ok" | "error";

export function WaitlistForm() {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [company, setCompany] = useState("");
  const [role, setRole] = useState<(typeof WAITLIST_ROLES)[number]>("security_engineer");
  const [message, setMessage] = useState("");
  const [website, setWebsite] = useState("");
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string | null>(null);

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setStatus("busy");
    try {
      const res = await fetch("/api/waitlist", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name, email, company, role, message, website }),
      });
      const body = (await res.json()) as { ok?: boolean; error?: string };
      if (!res.ok || !body.ok) {
        setStatus("error");
        setError(body.error ?? "Could not submit. Try again.");
        return;
      }
      setStatus("ok");
      analytics.marketing_cta_clicked("waitlist_submitted");
    } catch {
      setStatus("error");
      setError("Could not submit. Try again.");
    }
  }

  return (
    <div className="relative flex min-h-dvh flex-col items-center justify-center px-4 py-16">
      <div className="w-full max-w-md">
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
          {status === "ok" ? (
            <div>
              <h1 className="text-lg font-semibold tracking-tight">You are on the list</h1>
              <p className="mt-2 text-sm text-muted-foreground">
                I will email you at <span className="text-foreground">{email}</span> when there is
                a seat. The public demo on the homepage still runs without an account.
              </p>
              <Link
                href="/#try"
                className="mt-6 inline-flex h-9 items-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground"
              >
                Try a live verdict
              </Link>
            </div>
          ) : (
            <>
              <h1 className="text-lg font-semibold tracking-tight">Request access</h1>
              <p className="mt-1 text-sm text-muted-foreground">
                The workbench is invite-only. Tell me who you are and I will follow up from
                admin@homaid.dev.
              </p>

              <form onSubmit={onSubmit} className="mt-6 space-y-4">
                <Field label="Name" htmlFor="name">
                  <input
                    id="name"
                    name="name"
                    autoComplete="name"
                    required
                    minLength={2}
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    className={fieldClass}
                  />
                </Field>
                <Field label="Work email" htmlFor="email">
                  <input
                    id="email"
                    name="email"
                    type="email"
                    autoComplete="email"
                    required
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    className={fieldClass}
                  />
                </Field>
                <Field label="Company" htmlFor="company">
                  <input
                    id="company"
                    name="company"
                    autoComplete="organization"
                    value={company}
                    onChange={(e) => setCompany(e.target.value)}
                    className={fieldClass}
                  />
                </Field>
                <Field label="Role" htmlFor="role">
                  <select
                    id="role"
                    name="role"
                    value={role}
                    onChange={(e) =>
                      setRole(e.target.value as (typeof WAITLIST_ROLES)[number])
                    }
                    className={fieldClass}
                  >
                    {WAITLIST_ROLES.map((r) => (
                      <option key={r} value={r}>
                        {ROLE_LABELS[r]}
                      </option>
                    ))}
                  </select>
                </Field>
                <Field label="Why do you want to try Attest?" htmlFor="message">
                  <textarea
                    id="message"
                    name="message"
                    required
                    minLength={10}
                    rows={4}
                    value={message}
                    onChange={(e) => setMessage(e.target.value)}
                    className={`${fieldClass} h-auto py-2`}
                    placeholder="CloudTrail volume, air-gapped SOC, evaluating signed agent verdicts…"
                  />
                </Field>
                <div className="hidden" aria-hidden="true">
                  <label htmlFor="website">Website</label>
                  <input
                    id="website"
                    name="website"
                    tabIndex={-1}
                    autoComplete="off"
                    value={website}
                    onChange={(e) => setWebsite(e.target.value)}
                  />
                </div>

                {error ? (
                  <p className="text-xs text-destructive" role="alert">
                    {error}
                  </p>
                ) : null}

                <button
                  type="submit"
                  disabled={status === "busy"}
                  className="inline-flex h-9 w-full items-center justify-center gap-2 rounded-md bg-primary text-sm font-medium text-primary-foreground disabled:opacity-60"
                >
                  {status === "busy" ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                  Join the waitlist
                </button>
              </form>
            </>
          )}
        </div>

        <p className="mt-6 text-center text-xs text-muted-foreground">
          <Link href="/" className="underline underline-offset-4 hover:text-foreground">
            Back to the homepage
          </Link>
        </p>
      </div>
    </div>
  );
}

const fieldClass =
  "h-9 w-full rounded-md border border-border bg-background/80 px-3 text-sm outline-none focus:border-primary/50";

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <label htmlFor={htmlFor} className="text-xs text-muted-foreground">
        {label}
      </label>
      {children}
    </div>
  );
}
