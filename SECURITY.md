# Security Policy

Attest is a security product, so reports about it are taken seriously even while it is pre-1.0.

## Supported versions

| Version | Supported |
| ------- | --------- |
| `main`  | Yes       |
| Tags before 1.0 | No — fixes land on `main` only |

## Reporting a vulnerability

**Please do not open a public issue.**

1. Preferred: use GitHub's private reporting —
   [Report a vulnerability](https://github.com/mhomaid/attest/security/advisories/new).
2. Alternatively, email **Mohamed Homaid** at **mhomaid@gmail.com** with the subject
   `attest security`.

Include the affected component (crate or app), a description of the impact, and steps or a
proof of concept. You should get an acknowledgement within 3 business days and a triage
decision within 10. Fixes are credited in the release notes unless you ask otherwise.

## Scope

In scope:

- **Attestation integrity** — forging, altering, or replaying an `AttestationEnvelope` so that
  it still verifies; weaknesses in canonicalisation or Ed25519 signing
  (`crates/attest-attestation`).
- **Agent tool boundary** — bypassing the MCP gateway's tool policy, or getting an agent to
  call a tool or reach data it is not permitted to (`crates/attest-mcp-gateway`,
  `crates/attest-orchestrator`).
- **Prompt injection that changes a verdict or disposition** — e.g. attacker-controlled log
  fields that cause an auto-close. Include the input event.
- **Workbench auth** — session forgery, auth bypass, tenant data leakage (`apps/workbench`).
- **Secrets** — any credential committed to this repository.

Out of scope:

- The local-development defaults below, when used locally.
- Denial of service against a single-node local `docker compose` stack.
- Findings that require an already-compromised host or signing key.

## Local-development defaults

`docker-compose.yml` and `apps/workbench/.env.local.example` ship credentials such as `minioadmin`, the Postgres
password `attest`, and a dev Better Auth secret. They exist so the stack runs with no setup and
must never be used in a deployment. Guards:

- The workbench **refuses to start in production** (`NODE_ENV=production`) unless
  `BETTER_AUTH_SECRET` is set to a unique value of at least 32 characters.
- The dev seed route (`/api/auth/seed-analyst`) returns 404 in production regardless of
  `ALLOW_AUTH_SEED`.
- The orchestrator reads its attestation signing key from `ATTEST_SIGNING_KEY` (32-byte hex
  seed). If unset it generates an ephemeral key per process and logs the verifying key — fine
  for development, useless for audit. Set and protect a stable key for any deployment whose
  attestations you intend to verify.

## Attestation guarantees (and limits)

Envelopes are Ed25519-signed over the SHA-256 of a sorted-key canonical JSON body. A valid
signature proves the envelope was produced by a holder of the signing key and has not been
modified since.

Each envelope also carries `prev_hash`, the hash of the envelope before it. `attest verify`
walks that chain, so deleting, reordering, or duplicating an envelope in the middle of a log
fails verification. What the chain does **not** prove:

- **Truncation at the tail.** Dropping the most recent envelopes leaves a valid, shorter chain.
- **Rewrites by a key holder.** Anyone with `ATTEST_SIGNING_KEY` can re-sign a whole new chain.

Both need the chain tip anchored somewhere the operator cannot rewrite (a transparency log or
object-lock storage). That is on the roadmap. The optional S3 replica
(`ATTEST_LOG_S3_BUCKET`) is for durability, not tamper evidence.
