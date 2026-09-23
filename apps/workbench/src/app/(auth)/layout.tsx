import type { ReactNode } from "react";

export default function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <div className="marketing sf-grid-page relative min-h-dvh">{children}</div>
  );
}
