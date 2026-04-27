import Link from "next/link";

export default function HomePage() {
  return (
    <main className="grid min-h-screen place-items-center p-6">
      <section className="max-w-2xl rounded-xl border border-border bg-card/80 p-6 shadow-2xl">
        <div className="mb-3 font-mono text-xs uppercase tracking-[0.24em] text-primary">
          Attest
        </div>
        <h1 className="text-3xl font-semibold tracking-tight">
          Streaming-first, agent-aware security operations.
        </h1>
        <p className="mt-3 text-sm leading-6 text-muted-foreground">
          The first visual shell is available now as a mocked analyst workbench. Backend
          connections will replace the sample data as the Rust services come online.
        </p>
        <Link
          href="/workbench/queue"
          className="mt-5 inline-flex rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground"
        >
          Open Workbench Queue
        </Link>
      </section>
    </main>
  );
}
