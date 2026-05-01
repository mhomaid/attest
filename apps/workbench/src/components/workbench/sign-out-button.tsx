"use client";

import { LogOut } from "lucide-react";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { authClient } from "@/lib/auth-client";

export function SignOutButton() {
  const router = useRouter();
  const [busy, setBusy] = useState(false);

  async function signOut() {
    setBusy(true);
    try {
      await authClient.signOut();
    } finally {
      await fetch("/api/auth/force-logout", { method: "POST" });
      router.replace("/login");
      router.refresh();
    }
  }

  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      onClick={signOut}
      disabled={busy}
      className="h-8 px-2 text-muted-foreground hover:text-foreground"
      title="Sign out"
    >
      <LogOut className="h-4 w-4" />
      <span className="hidden md:inline">{busy ? "Signing out..." : "Sign out"}</span>
    </Button>
  );
}
