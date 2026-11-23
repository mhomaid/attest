import { defineConfig, devices } from "@playwright/test";

// The OIDC project needs a Keycloak with infra/keycloak/example-realm.json on :8080.
// CI starts one and sets ATTEST_E2E_OIDC=1; locally see apps/workbench/.env.local.example.
const oidcE2e = process.env.ATTEST_E2E_OIDC === "1";
const keycloak = process.env.ATTEST_E2E_KEYCLOAK_URL ?? "http://localhost:8080";

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
      ...(oidcE2e
        ? {
            ATTEST_OIDC_ISSUER: `${keycloak}/realms/example`,
            ATTEST_OIDC_CLIENT_ID: "attest-workbench",
            ATTEST_OIDC_CLIENT_SECRET: "e2e-client-secret",
            ATTEST_OIDC_ROLE_CLAIM: "roles",
            ATTEST_OIDC_LABEL: "Keycloak",
          }
        : {}),
    },
  },
  projects: [
    { name: "setup", testMatch: /auth\.setup\.ts/ },
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], storageState: "tests/e2e/.auth/user.json" },
      dependencies: ["setup"],
      testMatch: /.*\.spec\.ts/,
      testIgnore: /oidc\.spec\.ts/,
    },
    {
      name: "firefox",
      use: { ...devices["Desktop Firefox"], storageState: "tests/e2e/.auth/user.json" },
      dependencies: ["setup"],
      testMatch: /.*\.spec\.ts/,
      testIgnore: /oidc\.spec\.ts/,
    },
    {
      name: "webkit",
      use: { ...devices["Desktop Safari"], storageState: "tests/e2e/.auth/user.json" },
      dependencies: ["setup"],
      testMatch: /.*\.spec\.ts/,
      testIgnore: /oidc\.spec\.ts/,
    },
    ...(oidcE2e
      ? [
          {
            name: "oidc",
            use: { ...devices["Desktop Chrome"] },
            testMatch: /oidc\.spec\.ts/,
          },
        ]
      : []),
  ],
});
