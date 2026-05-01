import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: process.env.CI ? [["github"], ["line"]] : [["list"]],
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:3000",
    trace: "on-first-retry",
    javaScriptEnabled: true,
  },
  webServer: {
    command: "bun run dev -- -p 3000",
    url: "http://localhost:3000",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    env: {
      ...process.env,
      DATABASE_URL: process.env.DATABASE_URL ?? "postgres://attest:attest@127.0.0.1:5432/attest",
      BETTER_AUTH_SECRET:
        process.env.BETTER_AUTH_SECRET ?? "local-dev-better-auth-secret-min-32-chars!!",
      ALLOW_AUTH_SEED: process.env.ALLOW_AUTH_SEED ?? "1",
      AUTH_SEED_SECRET: process.env.AUTH_SEED_SECRET ?? "ci-auth-seed-secret",
    },
  },
  projects: [
    { name: "setup", testMatch: /auth\.setup\.ts/ },
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], storageState: "tests/e2e/.auth/user.json" },
      dependencies: ["setup"],
      testMatch: /.*\.spec\.ts/,
    },
    {
      name: "firefox",
      use: { ...devices["Desktop Firefox"], storageState: "tests/e2e/.auth/user.json" },
      dependencies: ["setup"],
      testMatch: /.*\.spec\.ts/,
    },
    {
      name: "webkit",
      use: { ...devices["Desktop Safari"], storageState: "tests/e2e/.auth/user.json" },
      dependencies: ["setup"],
      testMatch: /.*\.spec\.ts/,
    },
  ],
});
