import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const appRoot = path.resolve(import.meta.dir, "../..");

/**
 * Routes Next would compile into the standalone server that the production
 * image copies in (`COPY --from=builder .../.next/standalone`).
 */
function appRouteFiles(appDir: string): string[] {
  const files: string[] = [];
  const stack = [appDir];
  while (stack.length > 0) {
    const dir = stack.pop();
    if (!dir) break;
    for (const name of readdirSync(dir)) {
      const full = path.join(dir, name);
      if (statSync(full).isDirectory()) {
        stack.push(full);
        continue;
      }
      if (/^(page|route)\.(tsx|ts)$/.test(name)) files.push(full);
    }
  }
  return files;
}

function stageWorkbench(): string {
  const dest = mkdtempSync(path.join(tmpdir(), "attest-image-"));
  cpSync(path.join(appRoot, "src/app"), path.join(dest, "src/app"), { recursive: true });
  cpSync(path.join(appRoot, "railway.toml"), path.join(dest, "railway.toml"));
  cpSync(path.join(appRoot, "scripts"), path.join(dest, "scripts"), { recursive: true });
  return dest;
}

function applyProfile(dest: string, profile: "production" | "demo") {
  const result = spawnSync("bun", ["scripts/apply-build-profile.ts"], {
    cwd: dest,
    env: { ...process.env, ATTEST_BUILD_PROFILE: profile },
    encoding: "utf8",
  });
  expect(result.status, result.stderr || result.stdout).toBe(0);
}

describe("production image", () => {
  test("contains no waitlist route", () => {
    const dockerfile = readFileSync(path.join(appRoot, "Dockerfile"), "utf8");
    expect(dockerfile).toContain("ARG ATTEST_BUILD_PROFILE=production");
    const profileAt = dockerfile.indexOf("apply-build-profile");
    const buildAt = dockerfile.indexOf("bun run build");
    expect(profileAt).toBeGreaterThan(-1);
    expect(buildAt).toBeGreaterThan(profileAt);

    const runner = dockerfile.split("AS runner")[1] ?? "";
    expect(runner).not.toMatch(/railway/i);
    expect(runner).not.toMatch(/waitlist/i);
    expect(runner).not.toMatch(/\(marketing\)/);

    // Railway forwards service variables as Docker build args. The hosted demo
    // opts in here; the Dockerfile default stays production.
    const railwayDemo = readFileSync(
      path.resolve(appRoot, "../../infra/railway/workbench.json"),
      "utf8",
    );
    expect(railwayDemo).toMatch(/"ATTEST_BUILD_PROFILE"\s*:\s*"demo"/);

    const dockerignore = readFileSync(path.join(appRoot, ".dockerignore"), "utf8");
    expect(dockerignore).toMatch(/^railway\.toml$/m);
    expect(dockerignore).toMatch(/^railway\.json$/m);

    const dest = stageWorkbench();
    try {
      applyProfile(dest, "production");
      const routes = appRouteFiles(path.join(dest, "src/app"));
      const waitlist = routes.filter((file) => file.includes(`${path.sep}waitlist${path.sep}`));
      expect(waitlist).toEqual([]);
      expect(existsSync(path.join(dest, "src/app/(marketing)"))).toBe(false);
      expect(existsSync(path.join(dest, "railway.toml"))).toBe(false);
      expect(existsSync(path.join(dest, "src/app/api/demo"))).toBe(false);
    } finally {
      rmSync(dest, { recursive: true, force: true });
    }
  });

  test("demo profile keeps the waitlist route and marketing pages", () => {
    const dest = stageWorkbench();
    try {
      applyProfile(dest, "demo");
      expect(existsSync(path.join(dest, "src/app/(auth)/waitlist/page.tsx"))).toBe(true);
      expect(existsSync(path.join(dest, "src/app/api/waitlist/route.ts"))).toBe(true);
      expect(existsSync(path.join(dest, "src/app/(marketing)/page.tsx"))).toBe(true);
      expect(existsSync(path.join(dest, "railway.toml"))).toBe(true);
    } finally {
      rmSync(dest, { recursive: true, force: true });
    }
  });
});
