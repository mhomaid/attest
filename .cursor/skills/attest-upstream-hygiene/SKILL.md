---
name: attest-upstream-hygiene
description: Keep the public Attest repo free of organization-specific content (names, domains, hostnames, account IDs, stream names, incident IDs, real logs). Use before every commit and PR, when writing fixtures or docs, and whenever a request mentions a specific company's environment.
---

# Upstream hygiene

Attest is public. The people deploying it run private overlays with their own configuration.
Your job in this repo is to build the **generic extension point**, never the private details.

## Before every commit

```bash
.cursor/skills/attest-upstream-hygiene/scripts/check-org-leaks.sh
```

It reads a private deny-list from `.attest-denylist` (gitignored, local) or the
`ATTEST_ORG_DENYLIST` secret (CI) and also checks for real AWS account IDs in ARNs and private
keys. A hit means remove the content. Never add the term to an allow list, and never print
the deny-list terms in chat, commits or PR text.

## Translating a private request into public work

| The request says | Build upstream | Leave for the private overlay |
| --- | --- | --- |
| "Map our `store-security` stream's `merchant_id` field" | A config-driven field mapping (`mappings/*.yaml`) and an OCSF extension prefix setting | The actual mapping file |
| "Alert when our payment provider's webhook is blocked at the edge" | A generic rule: edge action = block/challenge on a configured list of callback paths | The list of paths and provider IPs |
| "Never let agents touch our VIP stores" | A do-not-touch list in the policy file, with tests | The list itself |
| "Use our SSO" | Generic OIDC with role mapping from claims | Issuer URL, client ID, group names |
| "Our incident on <date> looked like X" | A synthetic scenario that reproduces the pattern | Anything that identifies the incident |

## Fixtures

- Build events from vendor documentation schemas and generators (`tools/load-gen`).
- Use placeholders: `example.com`, `acme`, `tenant-a`, TEST-NET IPs (`192.0.2.0/24`,
  `198.51.100.0/24`, `203.0.113.0/24`), AWS docs account `123456789012`, user IDs like `1001`.
- No real log lines, even redacted. Redaction misses things.

## Commits and PRs

- Commit with your public identity and `-s` (DCO).
- Describe the generic capability: "add Graylog alert source", not why a specific company needs it.
