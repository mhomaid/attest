/**
 * Opens an authenticated analyst session before Lighthouse collects `/workbench/queue`.
 * Requires the workbench dev server (see `lighthouserc.cjs` `startServerCommand`).
 */
module.exports = async function lhciPuppeteerAuth(browser) {
  const page = await browser.newPage();
  const origin = process.env.LHCI_ORIGIN ?? "http://127.0.0.1:3000";
  const seedSecret = process.env.AUTH_SEED_SECRET ?? "ci-auth-seed-secret";

  await fetch(`${origin}/api/auth/seed-analyst`, {
    method: "POST",
    headers: { "x-seed-secret": seedSecret },
  }).catch(() => {});

  await page.goto(`${origin}/login`, { waitUntil: "networkidle0" });
  await page.type("#email", "analyst@attest.local");
  await page.type("#password", "analyst-dev");
  await page.click('button[type="submit"]');
  await page.waitForNavigation({ waitUntil: "networkidle0" }).catch(() => {});
  await page.goto(`${origin}/workbench/queue`, { waitUntil: "networkidle0" });
};
