import { headers } from "next/headers";
import { redirect } from "next/navigation";
import { auth } from "@/lib/auth";
import { oidcConfigFromEnv, passwordAuthEnabled } from "@/lib/auth-config";
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
  const oidc = oidcConfigFromEnv();
  const password = passwordAuthEnabled();
  return (
    <LoginForm
      callbackUrl={callbackUrl ?? "/workbench/queue"}
      showLocalHints={password && process.env.ALLOW_AUTH_SEED === "1"}
      passwordEnabled={password}
      sso={oidc ? { providerId: oidc.providerId, label: oidc.label } : null}
    />
  );
}
