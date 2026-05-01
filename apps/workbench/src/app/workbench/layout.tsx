import type { ReactNode } from "react";
import { PostHogPageview } from "@/components/shared/posthog-pageview";
import { AppShell } from "@/components/workbench/app-shell";

export default function WorkbenchLayout({ children }: { children: ReactNode }) {
  return (
    <AppShell>
      <PostHogPageview />
      {children}
    </AppShell>
  );
}
