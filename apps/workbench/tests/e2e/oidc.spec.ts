import { expect, test, type Page } from "@playwright/test";

/**
 * Signs in through a real Keycloak (realm `example`, see infra/keycloak/example-realm.json).
 * Runs only in the `oidc` project, which needs ATTEST_E2E_OIDC=1 and Keycloak on :8080.
 */
const PASSWORD = "e2e-password";

async function ssoSignIn(page: Page, username: string) {
  await page.goto("/login");
  await page.getByTestId("sso-sign-in").click();
  await page.waitForURL(/\/realms\/example\//, { timeout: 30_000 });
  await page.locator("#username").fill(username);
  await page.locator("#password").fill(PASSWORD);
  await page.locator("#kc-login").click();
  await page.waitForURL(/\/workbench\//, { timeout: 30_000 });
}

async function me(page: Page) {
  const res = await page.request.get("/api/auth/me");
  expect(res.status()).toBe(200);
  return (await res.json()) as { email: string; role: string; permissions: string[] };
}

function override(page: Page) {
  return page.request.post("/api/cases/e2e-case/override", {
    data: { verdict: "benign", reason: "e2e" },
  });
}

test.describe("OIDC sign-in (Keycloak)", () => {
  test("analyst signs in, reaches the workbench and may override", async ({ page }) => {
    await ssoSignIn(page, "analyst-user");
    await expect(page).toHaveURL(/\/workbench\//);
    const user = await me(page);
    expect(user.email).toBe("analyst-user@example.com");
    expect(user.role).toBe("analyst");
    expect(user.permissions).toContain("case.override");
    // Authorization passes; the orchestrator is not running in e2e, so anything but 401/403 is fine.
    expect([401, 403]).not.toContain((await override(page)).status());
  });

  test("viewer signs in but cannot perform an analyst action", async ({ page }) => {
    await ssoSignIn(page, "viewer-user");
    const user = await me(page);
    expect(user.role).toBe("viewer");
    expect(user.permissions).toEqual(["case.view"]);
    expect((await override(page)).status()).toBe(403);
  });

  test("auditor signs in and cannot override", async ({ page }) => {
    await ssoSignIn(page, "auditor-user");
    expect((await me(page)).role).toBe("auditor");
    expect((await override(page)).status()).toBe(403);
  });

  test("lead signs in and may override", async ({ page }) => {
    await ssoSignIn(page, "lead-user");
    expect((await me(page)).role).toBe("lead");
    expect([401, 403]).not.toContain((await override(page)).status());
  });

  test("anonymous requests are rejected", async ({ request }) => {
    expect((await request.get("/api/auth/me")).status()).toBe(401);
    expect((await request.post("/api/cases/e2e-case/override", { data: {} })).status()).toBe(401);
  });
});
