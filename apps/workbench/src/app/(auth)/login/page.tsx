import { headers } from "next/headers";
import { redirect } from "next/navigation";
import { auth } from "@/lib/auth";
import { LoginForm } from "./login-form";

export default async function LoginPage({
  searchParams,
}: {
  searchParams: Promise<{ callbackUrl?: string }>;
}) {
  const session = await auth.api.getSession({ headers: await headers() });
  if (session) {
    redirect("/workbench/queue");
  }

  const { callbackUrl } = await searchParams;
  return <LoginForm callbackUrl={callbackUrl ?? "/workbench/queue"} />;
}
