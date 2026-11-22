import { expect, test } from "@playwright/test";

const forbidden = /ocsf_event_id|tool_call_result|agent_reasoning/i;

test.describe("observability hygiene", () => {
  test("telemetry off sends nothing to a third-party host", async ({ page }) => {
    const thirdParty: string[] = [];
    page.on("request", (req) => {
      const host = new URL(req.url()).host;
      if (host === "localhost:3000" || host === "127.0.0.1:3000") return;
      thirdParty.push(req.url());
    });
    await page.goto("/");
    await page.waitForTimeout(500);
    expect(thirdParty, thirdParty.join("\n")).toEqual([]);
  });

  test("no forbidden substrings in PostHog/Sentry egress; no page errors", async ({ page }) => {
    const bodies: string[] = [];
    page.on("request", (req) => {
      const url = req.url();
      if (!url.includes("posthog") && !url.includes("sentry")) return;
      const postData = req.postData();
      if (postData) bodies.push(postData);
    });

    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));

    await page.goto("/workbench/e2e-fixtures/hybrid");
    await expect(page.getByTestId("classifier-evidence-panel")).toBeVisible();

    await page.waitForTimeout(1500);
    const blob = bodies.join("\n");
    expect(blob).not.toMatch(forbidden);
    expect(errors, `page errors: ${errors.join("; ")}`).toEqual([]);
  });
});
