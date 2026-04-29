import type { ReactNode } from "react";
import { Geist, Geist_Mono } from "next/font/google";
import { Providers } from "@/components/shared/providers";
import "./globals.css";

const geistSans = Geist({
  subsets: ["latin"],
  variable: "--font-geist-sans",
});

const geistMono = Geist_Mono({
  subsets: ["latin"],
  variable: "--font-geist-mono",
});

export const metadata = {
  title: "Attest — Verifiable Agentic SIEM",
  description:
    "Streaming-first security operations with agent-aware detection, signed agent reasoning, and a self-improving detection mesh. Explore the analyst workbench.",
  openGraph: {
    title: "Attest — Verifiable Agentic SIEM",
    description:
      "AI agents as first-class entities on both sides of the attack. Portable detections, OCSF + Iceberg, multi-agent SOC you can audit.",
  },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html
      lang="en"
      className={`${geistSans.variable} ${geistMono.variable} dark scroll-smooth`}
      data-scroll-behavior="smooth"
      suppressHydrationWarning
    >
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
