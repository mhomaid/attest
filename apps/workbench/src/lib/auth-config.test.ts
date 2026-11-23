import { describe, expect, test } from "bun:test";
import { oidcConfigFromEnv, passwordAuthEnabled } from "./auth-config";

describe("passwordAuthEnabled", () => {
  test("is on by default", () => {
    expect(passwordAuthEnabled({})).toBe(true);
  });
  test("ATTEST_AUTH_PASSWORD=off turns it off", () => {
    expect(passwordAuthEnabled({ ATTEST_AUTH_PASSWORD: "off" })).toBe(false);
    expect(passwordAuthEnabled({ ATTEST_AUTH_PASSWORD: " OFF " })).toBe(false);
  });
  test("any other value leaves it on", () => {
    expect(passwordAuthEnabled({ ATTEST_AUTH_PASSWORD: "on" })).toBe(true);
    expect(passwordAuthEnabled({ ATTEST_AUTH_PASSWORD: "" })).toBe(true);
  });
});

describe("oidcConfigFromEnv", () => {
  const base = {
    ATTEST_OIDC_ISSUER: "https://idp.example.com/realms/example",
    ATTEST_OIDC_CLIENT_ID: "attest-workbench",
    ATTEST_OIDC_CLIENT_SECRET: "not-a-real-secret",
  };

  test("is null when nothing is set", () => {
    expect(oidcConfigFromEnv({})).toBeNull();
  });

  test("is null when a required value is missing", () => {
    expect(oidcConfigFromEnv({ ...base, ATTEST_OIDC_CLIENT_SECRET: "" })).toBeNull();
    expect(oidcConfigFromEnv({ ...base, ATTEST_OIDC_ISSUER: undefined })).toBeNull();
  });

  test("reads issuer, client and secret, and derives discovery from the issuer", () => {
    const cfg = oidcConfigFromEnv(base);
    expect(cfg).toMatchObject({
      providerId: "oidc",
      issuer: "https://idp.example.com/realms/example",
      clientId: "attest-workbench",
      clientSecret: "not-a-real-secret",
      discoveryUrl: "https://idp.example.com/realms/example/.well-known/openid-configuration",
      roleClaim: "role",
      defaultRole: "viewer",
    });
    expect(cfg?.scopes).toEqual(["openid", "profile", "email"]);
  });

  test("trailing slashes on the issuer do not double up", () => {
    const cfg = oidcConfigFromEnv({ ...base, ATTEST_OIDC_ISSUER: "https://idp.example.com/" });
    expect(cfg?.discoveryUrl).toBe("https://idp.example.com/.well-known/openid-configuration");
  });

  test("role claim, scopes, discovery URL and label are overridable", () => {
    const cfg = oidcConfigFromEnv({
      ...base,
      ATTEST_OIDC_ROLE_CLAIM: "realm_access.roles",
      ATTEST_OIDC_SCOPES: "openid email groups",
      ATTEST_OIDC_DISCOVERY_URL: "https://idp.example.com/custom/discovery",
      ATTEST_OIDC_LABEL: "Acme SSO",
    });
    expect(cfg?.roleClaim).toBe("realm_access.roles");
    expect(cfg?.scopes).toEqual(["openid", "email", "groups"]);
    expect(cfg?.discoveryUrl).toBe("https://idp.example.com/custom/discovery");
    expect(cfg?.label).toBe("Acme SSO");
  });

  test("an invalid default role is ignored (never escalates)", () => {
    expect(oidcConfigFromEnv({ ...base, ATTEST_OIDC_DEFAULT_ROLE: "root" })?.defaultRole).toBe(
      "viewer",
    );
    expect(oidcConfigFromEnv({ ...base, ATTEST_OIDC_DEFAULT_ROLE: "analyst" })?.defaultRole).toBe(
      "analyst",
    );
  });
});
