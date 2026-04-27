import type { ReactNode } from "react";

export const metadata = {
  title: "Attest",
  description: "Agent-aware security operations workbench",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
