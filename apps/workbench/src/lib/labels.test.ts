import { describe, expect, test } from "bun:test";
import {
  LABEL_CSV_HEADER,
  labelsToCsv,
  parseLabelBody,
  toAnalystLabel,
  type AnalystLabel,
} from "./labels";

const ENVELOPE = "11111111-1111-4111-8111-111111111111";

function label(overrides: Partial<AnalystLabel> = {}): AnalystLabel {
  return {
    id: "22222222-2222-4222-8222-222222222222",
    case_id: "33333333-3333-4333-8333-333333333333",
    envelope_id: ENVELOPE,
    agreement: "disagree",
    disposition: "benign",
    reason: "travel for tenant-a, analyst said \"no\"",
    user_id: "user-1001",
    user_email: "analyst@example.com",
    created_at: "2026-11-22T18:00:00.000Z",
    ...overrides,
  };
}

describe("parseLabelBody", () => {
  test("accepts agree and disagree with a disposition and a reason", () => {
    for (const agreement of ["agree", "disagree"] as const) {
      const parsed = parseLabelBody({
        envelope_id: ENVELOPE.toUpperCase(),
        agreement,
        disposition: "true_positive",
        reason: "  matches the signed verdict  ",
        user_id: "ignored",
        created_at: "1999-01-01T00:00:00.000Z",
      });
      expect(parsed.ok).toBe(true);
      if (parsed.ok) {
        expect(parsed.value.agreement).toBe(agreement);
        expect(parsed.value.envelope_id).toBe(ENVELOPE);
        expect(parsed.value.reason).toBe("matches the signed verdict");
        expect(parsed.value).not.toHaveProperty("user_id");
      }
    }
  });

  test("rejects a missing reason, a bad agreement and a bad disposition", () => {
    expect(parseLabelBody({ envelope_id: ENVELOPE, agreement: "agree", disposition: "benign" }).ok).toBe(
      false,
    );
    expect(
      parseLabelBody({
        envelope_id: ENVELOPE,
        agreement: "maybe",
        disposition: "benign",
        reason: "no",
      }).ok,
    ).toBe(false);
    expect(
      parseLabelBody({
        envelope_id: ENVELOPE,
        agreement: "agree",
        disposition: "closed",
        reason: "no",
      }).ok,
    ).toBe(false);
    expect(
      parseLabelBody({
        envelope_id: "not-an-envelope",
        agreement: "agree",
        disposition: "benign",
        reason: "no",
      }).ok,
    ).toBe(false);
  });
});

describe("labelsToCsv", () => {
  test("exports user, time and envelope id, quoting commas and quotes", () => {
    const row = label();
    const csv = labelsToCsv([row]);
    expect(csv.startsWith(`${LABEL_CSV_HEADER.join(",")}\n`)).toBe(true);
    expect(csv).toContain(`"${row.envelope_id}"`);
    expect(csv).toContain(`"${row.user_id}"`);
    expect(csv).toContain(`"${row.user_email}"`);
    expect(csv).toContain(`"${row.created_at}"`);
    expect(csv).toContain('"travel for tenant-a, analyst said ""no"""');
    expect(csv.endsWith("\n")).toBe(true);
  });

  test("an empty export is the header only", () => {
    expect(labelsToCsv([])).toBe(`${LABEL_CSV_HEADER.join(",")}\n`);
  });
});

describe("toAnalystLabel", () => {
  test("keeps the stored user, time and envelope id", () => {
    const row = toAnalystLabel({
      id: "22222222-2222-4222-8222-222222222222",
      case_id: "case-1001",
      envelope_id: ENVELOPE,
      agreement: "agree",
      disposition: "false_positive",
      reason: "expected login",
      user_id: "user-1001",
      user_email: "lead@example.com",
      created_at: "2026-11-22T18:04:00.000Z",
    });
    expect(row.user_id).toBe("user-1001");
    expect(row.user_email).toBe("lead@example.com");
    expect(row.created_at).toBe("2026-11-22T18:04:00.000Z");
    expect(row.envelope_id).toBe(ENVELOPE);
    expect(row.agreement).toBe("agree");
  });
});
