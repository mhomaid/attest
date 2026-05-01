import { LoaderCircle } from "lucide-react";

/**
 * Shown by Next.js `loading.tsx` while the case Server Component renders.
 * The RSC now completes in <150ms (event + baseline only), so this skeleton
 * appears very briefly. Keep it lightweight.
 */
export function CaseDetailLoading() {
  return (
    <div
      role="status"
      aria-live="polite"
      aria-busy="true"
      className="flex flex-1 flex-col gap-4 p-4 @lg/main:p-6"
    >
      <div className="rounded-xl border border-border bg-card/90 px-4 py-5 shadow-sm">
        <div className="flex items-start gap-3">
          <div className="grid h-11 w-11 shrink-0 place-items-center rounded-lg border border-primary/25 bg-primary/10">
            <LoaderCircle className="h-5 w-5 animate-spin text-primary" aria-hidden />
          </div>
          <div className="min-w-0 flex-1 space-y-2 pt-1">
            <div className="h-4 w-40 animate-pulse rounded bg-secondary" />
            <div className="h-3 w-72 animate-pulse rounded bg-secondary/70" />
          </div>
          <div className="h-14 w-20 animate-pulse rounded-md bg-secondary/60" />
        </div>

        <div className="mt-5 space-y-3 border-t border-border/80 pt-4">
          {[1, 2, 3].map((i) => (
            <div key={i} className="flex gap-3">
              <div className="h-6 w-6 shrink-0 animate-pulse rounded-full bg-secondary" />
              <div className="flex-1 space-y-1.5">
                <div className="h-3 w-32 animate-pulse rounded bg-secondary" />
                <div className="h-3 w-56 animate-pulse rounded bg-secondary/70" />
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
