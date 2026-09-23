import { betterAuth } from "better-auth";
import { pool } from "@/lib/db";

const baseURL =
  process.env.BETTER_AUTH_URL ??
  process.env.NEXT_PUBLIC_APP_URL ??
  "http://localhost:3000";

const DEV_AUTH_SECRET = "local-dev-better-auth-secret-min-32-chars!!";

/**
 * The dev secret is public (it's in this repo), so a production server must never sign
 * sessions with it. `next build` also runs with NODE_ENV=production, hence the phase check.
 */
function resolveAuthSecret(): string {
  const secret = process.env.BETTER_AUTH_SECRET;
  const productionRuntime =
    process.env.NODE_ENV === "production" &&
    process.env.NEXT_PHASE !== "phase-production-build";
  if (!productionRuntime) return secret ?? DEV_AUTH_SECRET;
  if (!secret || secret === DEV_AUTH_SECRET || secret.length < 32) {
    throw new Error(
      "BETTER_AUTH_SECRET must be set to a unique value of at least 32 characters in production",
    );
  }
  return secret;
}

const authSecret = resolveAuthSecret();

export const auth = betterAuth({
  baseURL,
  secret: authSecret,
  database: pool,
  user: {
    modelName: "auth_users",
    fields: {
      emailVerified: "email_verified",
      createdAt: "created_at",
      updatedAt: "updated_at",
    },
  },
  session: {
    modelName: "auth_sessions",
    fields: {
      userId: "user_id",
      expiresAt: "expires_at",
      ipAddress: "ip_address",
      userAgent: "user_agent",
      createdAt: "created_at",
      updatedAt: "updated_at",
    },
  },
  account: {
    modelName: "auth_accounts",
    fields: {
      userId: "user_id",
      accountId: "account_id",
      providerId: "provider_id",
      accessToken: "access_token",
      refreshToken: "refresh_token",
      accessTokenExpiresAt: "access_token_expires_at",
      refreshTokenExpiresAt: "refresh_token_expires_at",
      idToken: "id_token",
      createdAt: "created_at",
      updatedAt: "updated_at",
    },
  },
  verification: {
    modelName: "auth_verifications",
    fields: {
      expiresAt: "expires_at",
      createdAt: "created_at",
      updatedAt: "updated_at",
    },
  },
  emailAndPassword: {
    enabled: true,
    requireEmailVerification: false,
    minPasswordLength: 8,
  },
  trustedOrigins: [
    baseURL,
    "https://attest.homaid.dev",
    "https://attest-wb.up.railway.app",
  ],
});
