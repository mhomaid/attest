/**
 * Workbench roles and the permission matrix.
 *
 * Roles come from an OIDC claim (see `auth-config.ts`) and are stored on the user row at
 * sign-in. Server routes call `requirePermission` (see `authz.ts`); the UI never decides.
 */
export const ROLES = ["viewer", "analyst", "lead", "auditor"] as const;
export type Role = (typeof ROLES)[number];

export const PERMISSIONS = ["case.view", "case.override", "audit.read"] as const;
export type Permission = (typeof PERMISSIONS)[number];

const MATRIX: Record<Role, readonly Permission[]> = {
  viewer: ["case.view"],
  analyst: ["case.view", "case.override"],
  lead: ["case.view", "case.override", "audit.read"],
  auditor: ["case.view", "audit.read"],
};

/** Highest first. Used when an identity provider sends several roles. */
const PRECEDENCE: readonly Role[] = ["lead", "analyst", "auditor", "viewer"];

export function parseRole(value: unknown): Role | undefined {
  if (typeof value !== "string") return undefined;
  const v = value.trim().toLowerCase();
  return (ROLES as readonly string[]).includes(v) ? (v as Role) : undefined;
}

export function can(role: Role | undefined, permission: Permission): boolean {
  if (!role) return false;
  const allowed = MATRIX[role];
  return allowed ? allowed.includes(permission) : false;
}

const BLOCKED_KEYS = new Set(["__proto__", "constructor", "prototype"]);

function lookup(claims: Record<string, unknown>, path: string): unknown {
  if (Object.hasOwn(claims, path)) return claims[path];
  let cur: unknown = claims;
  for (const part of path.split(".")) {
    if (BLOCKED_KEYS.has(part) || typeof cur !== "object" || cur === null) return undefined;
    if (!Object.hasOwn(cur, part)) return undefined;
    cur = (cur as Record<string, unknown>)[part];
  }
  return cur;
}

/**
 * Resolve a role from ID-token/userinfo claims. `claimPath` may be a plain claim name or a
 * dotted path such as `realm_access.roles`. Unknown values never grant anything; if no known
 * role is present the `fallback` is used (viewer unless configured otherwise).
 */
export function roleFromClaims(
  claims: Record<string, unknown>,
  claimPath: string,
  fallback: Role = "viewer",
): Role {
  const raw = lookup(claims, claimPath);
  const values = Array.isArray(raw) ? raw : [raw];
  const found = new Set<Role>();
  for (const v of values) {
    const r = parseRole(v);
    if (r) found.add(r);
  }
  return PRECEDENCE.find((r) => found.has(r)) ?? fallback;
}
