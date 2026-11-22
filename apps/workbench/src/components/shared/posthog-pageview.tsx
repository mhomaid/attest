"use client";

import { usePathname, useSearchParams } from "next/navigation";
import { Suspense, useEffect } from "react";
import { posthog } from "@/lib/posthog";
import { shouldInitPosthog } from "@/lib/telemetry";

function PostHogPageviewInner() {
  const pathname = usePathname();
  const search = useSearchParams();

  useEffect(() => {
    if (!shouldInitPosthog()) return;
    if (!pathname) return;
    let url = `${window.origin}${pathname}`;
    const q = search?.toString();
    if (q) url += `?${q}`;
    posthog.capture("$pageview", { $current_url: url });
  }, [pathname, search]);

  return null;
}

export function PostHogPageview() {
  return (
    <Suspense fallback={null}>
      <PostHogPageviewInner />
    </Suspense>
  );
}
