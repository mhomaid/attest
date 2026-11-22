/**
 * Apply ATTEST_BUILD_PROFILE before `next build`.
 *
 * `production` (the Dockerfile default) deletes hosted-demo routes so they are
 * not compiled into the standalone server the production image copies.
 * `demo` (or unset) leaves the checkout alone. Run the production profile only
 * against a build copy — it deletes files next to this script.
 */
import { rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** Present in the hosted demo, absent from the production image. */
const DEMO_ONLY = [
  "src/app/(marketing)",
  "src/app/(auth)/waitlist",
  "src/app/api/waitlist",
  "src/app/api/demo",
  "src/app/llms.txt",
  "src/app/llms-full.txt",
  "src/app/opengraph-image.tsx",
  "railway.toml",
] as const;

const PRODUCTION_HOME = `import { redirect } from "next/navigation";

export default function HomePage() {
  redirect("/login");
}
`;

const profile = (process.env.ATTEST_BUILD_PROFILE ?? "demo").trim().toLowerCase();

if (profile === "production") {
  for (const rel of DEMO_ONLY) {
    await rm(path.join(root, rel), { recursive: true, force: true });
  }

  const home = path.join(root, "src/app/page.tsx");
  try {
    await stat(home);
  } catch {
    await writeFile(home, PRODUCTION_HOME, "utf8");
  }
}
