import { z } from "zod";

export const WAITLIST_ROLES = [
  "security_engineer",
  "ciso",
  "founder",
  "investor",
  "researcher",
  "other",
] as const;

export const waitlistSchema = z.object({
  name: z.string().trim().min(2).max(80),
  email: z.email().max(160),
  company: z.string().trim().max(120).optional().or(z.literal("")),
  role: z.enum(WAITLIST_ROLES),
  message: z.string().trim().min(10).max(2000),
  /** Honeypot — bots fill this; the API drops the request. */
  website: z.string().max(200).optional().or(z.literal("")),
});

export type WaitlistInput = z.infer<typeof waitlistSchema>;

export const ROLE_LABELS: Record<(typeof WAITLIST_ROLES)[number], string> = {
  security_engineer: "Security engineer",
  ciso: "CISO / security leader",
  founder: "Founder / operator",
  investor: "Investor",
  researcher: "Researcher",
  other: "Other",
};

export function formatWaitlistEmail(input: WaitlistInput, ip: string): {
  subject: string;
  text: string;
} {
  const company = input.company?.trim() || "—";
  const subject = `Attest waitlist: ${input.name} (${company})`;
  const text = [
    "Someone requested access to Attest.",
    "",
    `Name:     ${input.name}`,
    `Email:    ${input.email}`,
    `Company:  ${company}`,
    `Role:     ${ROLE_LABELS[input.role]}`,
    `IP:       ${ip}`,
    "",
    "Why they want to try it:",
    input.message,
    "",
    "Reply to this email to reach them.",
  ].join("\n");
  return { subject, text };
}
