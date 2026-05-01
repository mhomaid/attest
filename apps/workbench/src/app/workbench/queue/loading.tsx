export default function QueueLoading() {
  return (
    <div className="space-y-3 p-3">
      {/* Header card skeleton */}
      <section className="rounded-lg border border-border bg-card/80 p-3 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="flex-1 space-y-2">
            <div className="flex gap-2">
              <div className="h-5 w-14 animate-pulse rounded bg-muted" />
              <div className="h-5 w-36 animate-pulse rounded bg-muted" />
            </div>
            <div className="h-6 w-48 animate-pulse rounded bg-muted" />
            <div className="h-4 w-80 animate-pulse rounded bg-muted" />
          </div>
          <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
            {[...Array(4)].map((_, i) => (
              <div key={i} className="rounded-md border border-border bg-background/60 px-3 py-2">
                <div className="h-6 w-8 animate-pulse rounded bg-muted" />
                <div className="mt-1 h-3 w-20 animate-pulse rounded bg-muted" />
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Table skeleton */}
      <section className="overflow-hidden rounded-lg border border-border bg-card/80 shadow-sm">
        <div className="flex items-center gap-2 border-b border-border px-3 py-2">
          <div className="h-4 w-32 animate-pulse rounded bg-muted" />
          <div className="ml-auto h-8 w-48 animate-pulse rounded bg-muted" />
        </div>
        <div className="divide-y divide-border">
          {[...Array(6)].map((_, i) => (
            <div key={i} className="flex items-center gap-4 px-4 py-3">
              <div className="h-5 w-16 animate-pulse rounded bg-muted" />
              <div className="h-4 flex-1 animate-pulse rounded bg-muted" />
              <div className="h-4 w-32 animate-pulse rounded bg-muted" />
              <div className="h-4 w-20 animate-pulse rounded bg-muted" />
              <div className="h-4 w-8 animate-pulse rounded bg-muted" />
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
