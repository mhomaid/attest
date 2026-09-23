import { describe, expect, test } from "bun:test";
import { formatWaitlistEmail, waitlistSchema } from "./waitlist";

describe("waitlistSchema", () => {
  const valid = {
    name: "Ada Lovelace",
    email: "ada@example.com",
    company: "Analytical Engines",
    role: "security_engineer" as const,
    message: "I want to try the signed triage loop on our CloudTrail.",
    website: "",
  };

  test("accepts a complete request", () => {
    expect(waitlistSchema.parse(valid).email).toBe("ada@example.com");
  });

  test("parses a honeypot fill so the API can drop it silently", () => {
    expect(waitlistSchema.parse({ ...valid, website: "https://spam.test" }).website).toBe(
      "https://spam.test",
    );
  });

  test("rejects a short message", () => {
    expect(waitlistSchema.safeParse({ ...valid, message: "hi" }).success).toBe(false);
  });

  test("formats a notify email", () => {
    const { subject, text } = formatWaitlistEmail(valid, "203.0.113.4");
    expect(subject).toContain("Ada Lovelace");
    expect(text).toContain("ada@example.com");
    expect(text).toContain("203.0.113.4");
  });
});
