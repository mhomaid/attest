import { describe, expect, test } from "bun:test";
import { can, parseRole, PERMISSIONS, ROLES, roleFromClaims } from "./roles";

describe("role matrix", () => {
  test("the four roles exist", () => {
    expect([...ROLES].sort()).toEqual(["analyst", "auditor", "lead", "viewer"]);
  });

  test("viewer can only read cases", () => {
    expect(can("viewer", "case.view")).toBe(true);
    expect(can("viewer", "case.override")).toBe(false);
    expect(can("viewer", "label.write")).toBe(false);
    expect(can("viewer", "audit.read")).toBe(false);
  });

  test("analyst can read, override and label but not read the audit trail", () => {
    expect(can("analyst", "case.view")).toBe(true);
    expect(can("analyst", "case.override")).toBe(true);
    expect(can("analyst", "label.write")).toBe(true);
    expect(can("analyst", "audit.read")).toBe(false);
  });

  test("lead can do everything", () => {
    for (const p of PERMISSIONS) expect(can("lead", p)).toBe(true);
  });

  test("auditor reads cases and the audit trail but cannot override or label", () => {
    expect(can("auditor", "case.view")).toBe(true);
    expect(can("auditor", "audit.read")).toBe(true);
    expect(can("auditor", "case.override")).toBe(false);
    expect(can("auditor", "label.write")).toBe(false);
  });

  test("an unknown role has no permissions", () => {
    expect(can("root" as never, "case.view")).toBe(false);
    expect(can(undefined, "case.view")).toBe(false);
  });
});

describe("parseRole", () => {
  test("accepts known roles case-insensitively", () => {
    expect(parseRole("Analyst")).toBe("analyst");
    expect(parseRole(" lead ")).toBe("lead");
  });
  test("rejects everything else", () => {
    expect(parseRole("admin")).toBeUndefined();
    expect(parseRole(42)).toBeUndefined();
    expect(parseRole(undefined)).toBeUndefined();
  });
});

describe("roleFromClaims", () => {
  test("reads a plain string claim", () => {
    expect(roleFromClaims({ role: "analyst" }, "role")).toBe("analyst");
  });

  test("reads an array claim", () => {
    expect(roleFromClaims({ roles: ["offline_access", "auditor"] }, "roles")).toBe("auditor");
  });

  test("reads a dotted path", () => {
    expect(roleFromClaims({ realm_access: { roles: ["lead"] } }, "realm_access.roles")).toBe(
      "lead",
    );
  });

  test("a literal dotted claim name wins over a path", () => {
    expect(roleFromClaims({ "acme.role": "analyst" }, "acme.role")).toBe("analyst");
  });

  test("missing claim falls back to the default (viewer)", () => {
    expect(roleFromClaims({}, "role")).toBe("viewer");
  });

  test("unknown values never grant a role", () => {
    expect(roleFromClaims({ role: "superadmin" }, "role")).toBe("viewer");
    expect(roleFromClaims({ role: ["superadmin", "root"] }, "role")).toBe("viewer");
  });

  test("several known roles resolve to the most privileged: lead > analyst > auditor > viewer", () => {
    expect(roleFromClaims({ roles: ["viewer", "analyst", "lead"] }, "roles")).toBe("lead");
    expect(roleFromClaims({ roles: ["auditor", "analyst"] }, "roles")).toBe("analyst");
    expect(roleFromClaims({ roles: ["viewer", "auditor"] }, "roles")).toBe("auditor");
  });

  test("the fallback is configurable", () => {
    expect(roleFromClaims({}, "role", "analyst")).toBe("analyst");
  });

  test("prototype keys are not followed", () => {
    expect(roleFromClaims({}, "__proto__.role")).toBe("viewer");
  });
});
