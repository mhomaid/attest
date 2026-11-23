import { notFound } from "next/navigation";
import { passwordAuthEnabled } from "@/lib/auth-config";
import { WaitlistForm } from "./waitlist-form";

export const metadata = {
  title: "Request access — Attest",
  description: "Join the Attest waitlist. The workbench is invite-only.",
};

export default function WaitlistPage() {
  if (!passwordAuthEnabled()) notFound();
  return <WaitlistForm />;
}
