# Attest task board

Source of truth for agent and human work. Plan: `docs/16_Production_Readiness_Plan.md`.
Process: the `attest-work-loop` skill. Definition of done:
`.cursor/skills/attest-work-loop/references/definition-of-done.md`.

Statuses: `todo` · `in_progress` · `review` · `done` · `blocked: decision` · `blocked: technical`.
Only one task is `in_progress` at a time per branch.

Model tiers (A deep, B standard, C fast) are defined in `MODELS.md`. `review: A` means a
tier-A model, ideally from a different vendor than the author, reviews the diff before `done`.

## Milestones

| Milestone | Goal | Tasks |
| --- | --- | --- |
| M0 Foundations | Repo is safe to build on in public | T-001 – T-005 |
| M1 Trust hardening | No unauthenticated path; keys and chain survive a hostile insider | T-010 – T-015 |
| M2 Sources and context | Real alerts in, real context for agents | T-020 – T-027 |
| M3 Triage, shadow mode, outputs | Analysts can run it in shadow and give labels | T-030 – T-035 |
| M4 Packaging | One-org production deploy on Kubernetes | T-040 – T-043 |

---

## M0 Foundations

### T-001 — Add the org leak check to CI and pre-commit
- status: done
- milestone: M0
- area: infra
- model tier: C
- depends: —
- plan ref: §2

**Why:** The repo is public and will receive work driven by private deployments. A leak must fail fast.

**Acceptance criteria**
- [x] `check-org-leaks.sh` exits 1 when a planted deny-list term is in a tracked file, and prints only `file:line` — test: `scripts/tests/leak_check_test.sh`
- [x] Exits 1 on an AWS account ID in an ARN other than `123456789012` — same test
- [x] Exits 0 on the current tree with an empty deny-list outside CI
- [x] `.github/workflows/org-leak-check.yml` runs on PRs; fork PRs run generic checks only
- [x] `.attest-denylist` added to `.gitignore`; `CONTRIBUTING.md` explains the check
- [x] Optional pre-commit hook documented (`ln -s ../../.cursor/skills/attest-upstream-hygiene/scripts/check-org-leaks.sh .git/hooks/pre-commit`)

**Plan:**
- Keep `check-org-leaks.sh` as the scanner. Prove it with `scripts/tests/leak_check_test.sh` in a temporary git repo so the real tree is never planted.
- Build the private-key fixture in pieces so the test file itself does not match the scanner.
- Run that test from `.github/workflows/org-leak-check.yml`, then the scanner. Fork PRs set `ATTEST_LEAK_CHECK_OPTIONAL`.
- Document `.attest-denylist`, the GitHub secret, and the optional pre-commit link in `CONTRIBUTING.md`. `make check-hygiene` is the local command.

**Evidence:** 2026-11-21
- Planted term, bad account `210987654321`, placeholder `123456789012`, repeated digits, private-key header, missing CI list, and fork opt-out: `scripts/tests/leak_check_test.sh` PASS. Hits are `file:line` only.
- Current tree with an empty deny-list outside CI: exit 0, same test.
- `VERIFY PASS mode=quick checks=1/1 skipped=0 commit=647f6e8 at=2026-11-21T10:06Z`
- `verify.sh full` was not run. This change does not touch Rust, TypeScript, Python, or infra code.

### T-002 — Replace MinIO in local dev and CI with a maintained S3-compatible store
- status: todo
- milestone: M0
- area: infra
- model tier: C
- depends: —
- plan ref: §4.4

**Why:** MinIO community edition is archived and no longer receives fixes. Attest only needs the S3 API.

**Acceptance criteria**
- [ ] `docker-compose.yml` uses SeaweedFS or Garage (ADR records the choice) with buckets created on start
- [ ] Iceberg warm-tier writes and ClickHouse `s3()` reads pass `make e2e-phase2`
- [ ] CI integration job starts the new service and the smoke step passes
- [ ] README and `infra/README.md` no longer reference MinIO as the default

**Plan:**

**Evidence:**

### T-003 — Make workbench telemetry opt-in and off by default
- status: todo
- milestone: M0
- area: workbench
- model tier: C
- depends: —
- plan ref: §5

**Why:** A SOC console shows log content. Nothing may leave the host unless the operator turns it on.

**Acceptance criteria**
- [ ] PostHog and Sentry initialize only when `ATTEST_TELEMETRY=on` and their keys are set — test: unit test on the init helpers
- [ ] With telemetry off, a Playwright run records zero requests to third-party hosts — test: network assertion in an e2e spec
- [ ] `env.example` documents the flag; default is off

**Plan:**

**Evidence:**

### T-004 — Detection test harness, and fix the brute-force rule
- status: todo
- milestone: M0
- area: heliql
- model tier: B
- depends: —
- plan ref: §6

**Why:** Rules need proof that they fire on the attack and stay quiet on the look-alike. `okta_brute_force.heliql` currently fires on any single failure.

**Acceptance criteria**
- [ ] A harness evaluates a rule against fixture event sequences without the full stack (in-memory evaluator or RisingWave in a test container) — test: `attest_heliql::harness::*`
- [ ] Every rule in `detections/` has one positive and one negative fixture, and both pass
- [ ] The brute-force rule counts failures per user per window with a threshold; the negative fixture (threshold − 1) does not fire
- [ ] CI runs the harness

**Plan:**

**Evidence:**

### T-005 — Agent context files and DCO
- status: todo
- milestone: M0
- area: docs
- model tier: C
- depends: —
- plan ref: §2

**Acceptance criteria**
- [ ] `AGENTS.md`, `.cursor/rules`, `.cursor/skills`, `.cursor/hooks.json`, `TASKS.md` and `docs/16_Production_Readiness_Plan.md` committed
- [x] `CONTRIBUTING.md` requires `Signed-off-by` (DCO); a DCO check runs on PRs
- [ ] `verify.sh quick` and `verify.sh full` run from a clean checkout

**Plan:**
- DCO text, `scripts/check-dco.sh`, `.github/workflows/dco.yml`, and `scripts/tests/dco_check_test.sh` landed with T-001.
- Still open: record `verify.sh quick` and `verify.sh full` from a clean checkout after this branch is committed. `quick` already passed once on the dirty tree (`VERIFY PASS mode=quick checks=1/1 skipped=0 commit=647f6e8 at=2026-11-21T10:06Z`).

**Evidence:**

---

## M1 Trust hardening

### T-010 — Service-to-service authentication
- status: todo
- milestone: M1
- area: platform
- model tier: A · review: A (second model)
- depends: T-005
- plan ref: §5.1

**Why:** Today control-plane, orchestrator, mcp-gateway and ws-gateway accept any caller, and collector ingest is open.

**Acceptance criteria**
- [ ] Every non-health route on those services requires a signed service token (short-lived JWT, audience per service) or mTLS; ADR records the choice
- [ ] One test per service: request without a token → 401; wrong audience → 401; valid → 200
- [ ] Collector HTTP ingest requires an HMAC signature over the body with a timestamp; replay older than 5 minutes → 401 — tests: `ingest_rejects_unsigned`, `ingest_rejects_replay`
- [ ] `/healthz` stays open; no secret appears in logs (test with a log capture)
- [ ] All e2e phases still pass with auth enabled

**Plan:**

**Evidence:**

### T-011 — Generic OIDC sign-in and roles in the workbench
- status: todo
- milestone: M1
- area: workbench
- model tier: B
- depends: T-003
- plan ref: §5.2

**Acceptance criteria**
- [ ] Better Auth generic OIDC provider configured from env (issuer, client ID, secret, role claim)
- [ ] Roles `viewer`, `analyst`, `lead`, `auditor` mapped from a claim; server routes enforce them — tests per role
- [ ] Email/password sign-up and the waitlist can be disabled by flag (`ATTEST_AUTH_PASSWORD=off`); default for production Helm values is off
- [ ] Playwright e2e signs in through a Keycloak container in CI

**Plan:**

**Evidence:**

### T-012 — Pluggable signer with a cloud KMS backend
- status: todo
- milestone: M1
- area: attestation
- model tier: A · review: A (second model)
- depends: —
- plan ref: §5.3

**Why:** `ATTEST_SIGNING_KEY` in an env var lets anyone holding it re-sign a chain.

**Acceptance criteria**
- [ ] `Signer` trait with `LocalKey` (current behaviour) and `AwsKms` implementations; KMS behind a cargo feature
- [ ] Envelope records `alg` and `key_id`; `attest verify` accepts a keyring (several public keys) and rejects an unknown `key_id` — tests
- [ ] Key rotation: envelopes signed before and after a rotation both verify — test
- [ ] Old envelopes without `key_id` still verify (backwards compatibility test)
- [ ] Check which signature algorithms the KMS supports; if Ed25519 is not available, use the supported algorithm and record it in `alg` (ADR)
- [ ] KMS path tested against LocalStack or a recorded mock

**Plan:**

**Evidence:**

### T-013 — Anchor the chain tip so tail truncation is detected
- status: todo
- milestone: M1
- area: attestation
- model tier: A · review: A (second model)
- depends: T-012
- plan ref: §5.3

**Acceptance criteria**
- [ ] Every N minutes (config) the orchestrator writes `{seq, tip_hash, signed_at, key_id}` signed to an object store prefix intended for object lock
- [ ] `attest verify --anchors <uri>` fails when rows after the last anchor are missing — test: delete the last 3 envelopes → verify fails with "truncated after seq X"
- [ ] Optional webhook posts each anchor to an external channel (off by default)
- [ ] Docs explain enabling S3 Object Lock in compliance mode

**Plan:**

**Evidence:**

### T-014 — Secrets interface
- status: todo
- milestone: M1
- area: platform
- model tier: B
- depends: —
- plan ref: §5.4

**Acceptance criteria**
- [ ] A `SecretSource` abstraction: env (dev), file (Kubernetes secret mount), AWS Secrets Manager (feature)
- [ ] Tool credentials are resolved per tool ID and reloaded without restart on rotation — test with the file source
- [ ] No production Helm value requires a secret in plain env

**Plan:**

**Evidence:**

### T-015 — Prompt-injection regression suite for log fields
- status: todo
- milestone: M1
- area: orchestrator
- model tier: A · review: A (second model)
- depends: —
- plan ref: §5.5

**Acceptance criteria**
- [ ] 20+ synthetic events with injection strings in user agent, URL, user name, resource name and free-text fields
- [ ] With a scripted mock LLM that obeys the injection, policy still denies out-of-role tools and the envelope records the denial — tests
- [ ] Classifier path is unaffected by text fields (features are numeric) — test
- [ ] Suite runs in CI without a real model

**Plan:**

**Evidence:**

---

## M2 Sources and context

### T-020 — Source plugin interface in the collector
- status: todo
- milestone: M2
- area: collector
- model tier: A · review: A (second model)
- depends: T-010
- plan ref: §4.1

**Acceptance criteria**
- [ ] `Source` trait (id, decode, normalize) as described in the `attest-add-source` skill; CloudTrail moved onto it
- [ ] Topics `attest.raw.<source>` and `attest.alerts`; topic names configurable with a prefix
- [ ] Every existing CloudTrail test passes unchanged; e2e phase 1 passes
- [ ] `FlatEvent` gains HTTP and network fields (path, status, src IP, ASN, disposition) without breaking RisingWave tables (migration included)

**Plan:**

**Evidence:**

### T-021 — Graylog alert source
- status: todo
- milestone: M2
- area: collector
- model tier: B
- depends: T-020
- plan ref: §4.2

**Acceptance criteria**
- [ ] Receives Graylog HTTP notifications (event + backlog) with HMAC; maps to OCSF Detection Finding (2004) with the event definition ID, title, priority, timerange and backlog references
- [ ] Synthetic fixtures built from Graylog's public notification format; normal, malformed and oversized cases tested
- [ ] Alerts land on `attest.alerts` and reach the orchestrator in an e2e test with a mock Graylog

**Plan:**

**Evidence:**

### T-022 — Graylog search tool and detection heartbeat
- status: todo
- milestone: M2
- area: gateway
- model tier: B · review: A
- depends: T-021, T-014
- plan ref: §4.2

**Acceptance criteria**
- [ ] `graylog_search` MCP tool: read-only token, bounded time range, row cap, timeout, rejects leading wildcards, returns message links — tests per the `attest-add-mcp-tool` skill
- [ ] Heartbeat poller compares each event definition's last execution with its schedule and emits a `detection_stalled` alert after a configurable grace — test with a mock API where one definition stops
- [ ] Policy: all roles may search; caps enforced; denial test for over-cap

**Plan:**

**Evidence:**

### T-023 — Cloudflare firewall events and audit log source
- status: todo
- milestone: M2
- area: collector
- model tier: B
- depends: T-020
- plan ref: §4.3

**Acceptance criteria**
- [ ] Logpush `firewall_events` (zone or account scoped) from object storage → OCSF HTTP Activity (4002) with action, rule ID, source, ray ID, client ASN and country
- [ ] Audit log API poller with cursor and backoff → OCSF API Activity (6003)
- [ ] Synthetic fixtures from Cloudflare's published field list; tests for block, challenge, skip and log actions
- [ ] `cloudflare_analytics` MCP tool (GraphQL, read-only token, caps)

**Plan:**

**Evidence:**

### T-024 — Config-driven field mapping and OCSF extension prefix
- status: todo
- milestone: M2
- area: collector
- model tier: B
- depends: T-020
- plan ref: §4.5

**Acceptance criteria**
- [ ] `mappings/*.yaml` maps source fields to OCSF paths or to `unmapped.<prefix>.<name>`; prefix set by config (default `ext`)
- [ ] Redaction rules in the same file: hash, drop or keep per field; card-number-like and email-like values hashed by default — tests
- [ ] Mapping errors are reported at startup, not at first event — test

**Plan:**

**Evidence:**

### T-025 — Real enrichment tools
- status: todo
- milestone: M2
- area: gateway
- model tier: B · review: A
- depends: T-014
- plan ref: §4.6

**Acceptance criteria**
- [ ] `ip_asn_lookup` from an offline MMDB file path (no network per lookup)
- [ ] `lookup_threat_intel` from synced open feeds (abuse.ch ThreatFox and URLhaus; optional MISP) with last-sync time in the output
- [ ] `get_user_baseline` reads real baselines from the hot tier; no hard-coded geography or hours
- [ ] With `ATTEST_DEMO_MODE` unset, no tool returns a `*_stub` source — test

**Plan:**

**Evidence:**

### T-026 — Generic entity lookup tool over read-only SQL templates
- status: todo
- milestone: M2
- area: gateway
- model tier: B · review: A
- depends: T-014
- plan ref: §4.6

**Why:** Deployers need "who owns this account / tenant / store" context without giving agents SQL.

**Acceptance criteria**
- [ ] `entity_lookup` runs only named, parameterized query templates from config against a read-only Postgres/MySQL connection
- [ ] Free-form SQL is impossible through the tool — test; parameters are bound, never interpolated — injection test
- [ ] Row cap, timeout, column allow list; redaction rules from T-024 apply

**Plan:**

**Evidence:**

### T-027 — AWS GuardDuty source
- status: todo
- milestone: M2
- area: collector
- model tier: C
- depends: T-020
- plan ref: §4.3

**Acceptance criteria**
- [ ] GuardDuty findings (EventBridge → S3 or HTTP) → OCSF Detection Finding with severity and resource
- [ ] Synthetic fixtures from AWS sample findings; tests

**Plan:**

**Evidence:**

---

## M3 Triage, shadow mode, outputs

### T-030 — Analyst labels in the workbench
- status: todo
- milestone: M3
- area: workbench
- model tier: B
- depends: T-011
- plan ref: §6.2

**Acceptance criteria**
- [ ] On each verdict an analyst records agree / disagree, the correct disposition and a reason; stored with user, time and envelope ID
- [ ] Labels export to Parquet/CSV for training — test
- [ ] Only `analyst` and `lead` can label — role tests

**Plan:**

**Evidence:**

### T-031 — Configurable features and retraining from labels
- status: todo
- milestone: M3
- area: ml
- model tier: B
- depends: T-030, T-024
- plan ref: §6.2

**Acceptance criteria**
- [ ] Feature set defined in config, versioned, and its column-order hash stored in the envelope (extends `column_order_hash`)
- [ ] `make train-classifier LABELS=<file>` trains, calibrates per detection, writes ONNX and its hash
- [ ] The P99 < 5 ms budget test still passes with the new feature count
- [ ] Model pinning rejects an un-pinned hash in CI — test

**Plan:**

**Evidence:**

### T-032 — Shadow mode and agreement metrics
- status: todo
- milestone: M3
- area: orchestrator
- model tier: B
- depends: T-030
- plan ref: §6.3

**Acceptance criteria**
- [ ] `ATTEST_MODE=shadow`: verdicts are signed and shown, auto-close and responder actions are recorded as `would_have` and never applied — tests
- [ ] `/v1/metrics/agreement` returns agreement per detection, missed true positives and auto-close candidates over a window
- [ ] Workbench page shows the same numbers

**Plan:**

**Evidence:**

### T-033 — Outputs: Slack, Jira, generic webhook, PagerDuty Events v2
- status: todo
- milestone: M3
- area: platform
- model tier: A · review: A (second model)
- depends: T-010
- plan ref: §6.4

**Acceptance criteria**
- [ ] Each output is a plugin with retries and a dead-letter topic; mock-server tests for success, 429 and 5xx
- [ ] Slack message has the verdict, evidence links and Approve/Reject buttons for planned actions; button callbacks are signature-verified — tests
- [ ] Approval records the approver identity in a new envelope linked to the planned action; approvals expire — tests

**Plan:**

**Evidence:**

### T-034 — Policy from file, with a do-not-touch list
- status: todo
- milestone: M3
- area: policy
- model tier: A · review: A (second model)
- depends: —
- plan ref: §5.6

**Acceptance criteria**
- [ ] Policies load from a YAML file (roles × tools × conditions); the current hard-coded rules become the default file and all existing policy tests pass against it
- [ ] Do-not-touch entities (users, accounts, IPs, ASNs, tenants) make any action on them `Escalate` or `Deny` — tests
- [ ] Invalid policy file fails startup with a clear error — test
- [ ] The loaded policy file's hash is recorded in every envelope

**Plan:**

**Evidence:**

### T-035 — Golden-set evaluation in CI
- status: todo
- milestone: M3
- area: ml
- model tier: B
- depends: T-031
- plan ref: §6.5

**Acceptance criteria**
- [ ] `eval/run_eval.py` runs the classifier and (with a mock LLM) the investigator over golden cases and fails on any missed true positive or a drop in agreement beyond a threshold
- [ ] Runs in CI; result attached as an artifact

**Plan:**

**Evidence:**

---

## M4 Packaging

### T-040 — Helm hardening for a single-organization production deploy
- status: todo
- milestone: M4
- area: infra
- model tier: B
- depends: T-010, T-014
- plan ref: §7

**Acceptance criteria**
- [ ] Values for external Kafka (SASL/IAM), ClickHouse, S3 and Postgres; bundled dev dependencies off by default
- [ ] NetworkPolicies (default deny, explicit allows), `securityContext` (non-root, read-only root FS), resource limits, PodDisruptionBudgets
- [ ] Service-account annotations for cloud IAM; no secrets in values
- [ ] `make helm-template` plus a kubeconform or similar schema check in CI

**Plan:**

**Evidence:**

### T-041 — Terraform module for a security account
- status: todo
- milestone: M4
- area: infra
- model tier: B
- depends: T-012, T-013
- plan ref: §7

**Acceptance criteria**
- [ ] AWS module creates KMS key, warm and anchor buckets with Object Lock, IAM roles for service accounts
- [ ] `make tf-validate` and `tflint` pass

**Plan:**

**Evidence:**

### T-042 — Single-organization deployment guide
- status: todo
- milestone: M4
- area: docs
- model tier: C
- depends: T-040, T-041
- plan ref: §7

**Acceptance criteria**
- [ ] `docs/17_Deploy_Single_Org.md`: prerequisites, overlay repo layout, configuration files, shadow-mode rollout, go-live gates, rollback
- [ ] Every config key referenced exists in `env.example` or Helm values (script check)

**Plan:**

**Evidence:**

### T-043 — Remove or isolate hosted-demo code paths
- status: todo
- milestone: M4
- area: platform
- model tier: C
- depends: —
- plan ref: §7

**Acceptance criteria**
- [ ] Railway config, waitlist and marketing pages are isolated behind a `demo` build profile or a separate app, and are not in the production image — test: production image contains no waitlist route
- [ ] README status table updated

**Plan:**

**Evidence:**
