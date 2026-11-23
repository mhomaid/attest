import { parseRole, type Role } from "./roles";

type Env = Record<string, string | undefined>;

/**
 * Email/password sign-up and the waitlist are on by default so local dev works. Production
 * Helm values set `ATTEST_AUTH_PASSWORD=off`.
 */
export function passwordAuthEnabled(env: Env = process.env): boolean {
  return env.ATTEST_AUTH_PASSWORD?.trim().toLowerCase() !== "off";
}

export type OidcConfig = {
  providerId: string;
  issuer: string;
  discoveryUrl: string;
  clientId: string;
  clientSecret: string;
  scopes: string[];
  roleClaim: string;
  defaultRole: Role;
  label: string;
};

/** Generic OIDC provider from env. Returns null unless issuer, client ID and secret are set. */
export function oidcConfigFromEnv(env: Env = process.env): OidcConfig | null {
  const issuer = env.ATTEST_OIDC_ISSUER?.trim();
  const clientId = env.ATTEST_OIDC_CLIENT_ID?.trim();
  const clientSecret = env.ATTEST_OIDC_CLIENT_SECRET?.trim();
  if (!issuer || !clientId || !clientSecret) return null;

  const scopes = (env.ATTEST_OIDC_SCOPES ?? "openid profile email").split(/[\s,]+/).filter(Boolean);
  return {
    providerId: "oidc",
    issuer,
    discoveryUrl:
      env.ATTEST_OIDC_DISCOVERY_URL?.trim() ||
      `${issuer.replace(/\/+$/, "")}/.well-known/openid-configuration`,
    clientId,
    clientSecret,
    scopes,
    roleClaim: env.ATTEST_OIDC_ROLE_CLAIM?.trim() || "role",
    defaultRole: parseRole(env.ATTEST_OIDC_DEFAULT_ROLE) ?? "viewer",
    label: env.ATTEST_OIDC_LABEL?.trim() || "Single sign-on",
  };
}
