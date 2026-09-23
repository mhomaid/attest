import { posthog } from "@/lib/posthog";

function capture(event: string, props?: Record<string, string | number | boolean>) {
  if (typeof window === "undefined") return;
  if (!process.env.NEXT_PUBLIC_POSTHOG_KEY) return;
  posthog.capture(event, props);
}

/** Typed product analytics — never include customer payload, raw OCSF, or reasoning text. */
export const analytics = {
  case_opened: () => capture("case_opened"),
  verdict_overridden: () => capture("verdict_overridden"),
  verdict_approved: () => capture("verdict_approved"),
  query_run: () => capture("query_run"),
  command_palette_used: () => capture("command_palette_used"),
  user_signed_in: (userId: string) => {
    if (typeof window === "undefined") return;
    if (!process.env.NEXT_PUBLIC_POSTHOG_KEY) return;
    posthog.identify(userId, { role: "analyst" });
    capture("user_signed_in");
  },
  login_failed: (reason: string) => capture("login_failed", { reason }),
  simulation_run: (props: { scenario_id: string; verdict: string; execution_path: string; latency_ms: number; escalated: boolean }) =>
    capture("simulation_run", props),
  batch_simulation_run: (props: { scenario_id: string; n: number; p95_ms: number; p99_ms: number; errors: number }) =>
    capture("batch_simulation_run", props),
  hunt_query_run: () => capture("hunt_query_run"),
  marketing_cta_clicked: (cta: string) => capture("marketing_cta_clicked", { cta }),
  marketing_quickstart_copied: (step: string) => capture("marketing_quickstart_copied", { step }),
  marketing_demo_step: (demo: "verify" | "guards", step: string) =>
    capture("marketing_demo_step", { demo, step }),
};
