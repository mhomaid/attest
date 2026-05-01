import { test as setup } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

const authFile = path.join(process.cwd(), "tests/e2e/.auth/user.json");

setup("seed analyst and sign in", async ({ page, request }) => {
  fs.mkdirSync(path.dirname(authFile), { recursive: true });

  if (process.env.ALLOW_AUTH_SEED === "1") {
    const secret = process.env.AUTH_SEED_SECRET ?? "ci-auth-seed-secret";
    const res = await request.post("/api/auth/seed-analyst", {
      headers: { "x-seed-secret": secret },
    });
    if (!res.ok()) {
      console.warn("seed-analyst:", res.status(), await res.text());
    }
  }

  await page.goto("/login");
  await page.getByLabel("Email").fill("analyst@attest.local");
  await page.getByLabel("Password").fill("analyst-dev");
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.waitForURL(/\/workbench\//, { timeout: 30_000 });
  await page.context().storageState({ path: authFile });
});
