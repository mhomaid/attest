import { expect, test } from "@playwright/test";

test.describe("workbench phase 8", () => {
  test("hybrid fixture shows classifier + LLM trace and SHAP", async ({ page }) => {
    await page.goto("/workbench/e2e-fixtures/hybrid");
    await expect(page.getByTestId("classifier-evidence-panel")).toBeVisible();
    await expect(page.getByTestId("llm-reasoning-timeline")).toBeVisible();
    const bars = page.getByTestId("shap-bar");
    await expect(bars.first()).toBeVisible();
    expect(await bars.count()).toBeGreaterThanOrEqual(1);
    await expect(page.locator('[data-testid="trace-step"][data-kind="tool_call"]')).toHaveCount(1);
  });

  test("override round-trip (mocked API)", async ({ page }) => {
    await page.route("**/api/cases/*/override", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          agent_action_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
          latency_ms: 12,
          signed_at: new Date().toISOString(),
        }),
      });
    });

    await page.goto("/workbench/e2e-fixtures/hybrid");
    await page.getByRole("button", { name: "Override verdict" }).click();
    await page.getByPlaceholder(/Confirmed benign/).fill("E2E override reason");
    await page.getByRole("button", { name: "Submit Override" }).click();
    await expect(page.getByText("Override recorded")).toBeVisible({ timeout: 5000 });
  });

  test("⌘K opens palette and navigates to coverage", async ({ page }) => {
    await page.goto("/workbench/queue");
    await page.keyboard.press("Meta+k");
    await page.getByPlaceholder("Search pages, alerts, detections").fill("Coverage");
    await page.getByText("MITRE ATT&CK coverage map").click();
    await expect(page).toHaveURL(/\/workbench\/coverage/);
  });

  test("document is dark by default", async ({ page }) => {
    await page.goto("/workbench/queue");
    const cls = await page.locator("html").getAttribute("class");
    expect(cls ?? "").toContain("dark");
  });
});
