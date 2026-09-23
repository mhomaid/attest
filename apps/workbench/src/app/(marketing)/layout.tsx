import type { ReactNode } from "react";
import { PostHogPageview } from "@/components/shared/posthog-pageview";

/** Sentinel-style shell: public landing only. Workbench uses the root body stack under `/workbench`. */
export default function MarketingLayout({ children }: { children: ReactNode }) {
  return (
    <div className="marketing sf-grid-page relative flex min-h-dvh flex-col">
      <PostHogPageview />
      <div
        className="pointer-events-none fixed inset-0 z-[1] overflow-hidden"
        aria-hidden
      >
        <div className="sf-scan-line absolute inset-x-0 top-0 flex h-10 w-full flex-col">
          <div className="h-px w-full bg-primary/45 shadow-[0_0_4px_1px_rgba(34,197,94,0.2),0_0_12px_2px_rgba(34,197,94,0.08)]" />
          <div className="h-full min-h-0 flex-1 bg-gradient-to-b from-primary/10 to-transparent" />
        </div>
      </div>
      <div className="relative z-0 flex min-h-dvh w-full flex-col">{children}</div>
    </div>
  );
}
