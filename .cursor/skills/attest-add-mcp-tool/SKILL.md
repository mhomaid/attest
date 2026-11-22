---
name: attest-add-mcp-tool
description: Add or replace a tool in attest-mcp-gateway (for example graylog_search, ip_asn_lookup, threat intel, entity lookup, Slack or Jira actions) with a policy entry per role, caps, redaction and negative tests. Use when agents need new data access or a stub tool must become real.
---

# Add an MCP gateway tool

Agents reach data only through the gateway. Every tool needs: a descriptor, an implementation,
a policy decision for every role, limits, and tests that prove the limits hold.

Current stubs to replace (in `crates/attest-mcp-gateway/src/tools.rs`): `lookup_threat_intel`,
`get_asset_context`, `get_user_baseline`, `analyze_code_snippet`, `sandbox_detonate`,
`propose_detection_pr`, `request_human_review`. Their outputs carry `"source": "*_stub"`.

## Steps

1. **Descriptor** in `registry.rs`: id, description the model sees, JSON schema for arguments.
   Keep the description factual. Do not tell the model to trust the output.
2. **Policy** in `attest-policy-engine`: decide Allow / Deny / Escalate for each `AgentRole`.
   Read-only lookups may be allowed for all roles. Anything that changes state outside Attest
   is `Escalate` (human approval) and only for `Responder`.
3. **Limits**: time range cap, row cap, timeout, rate per minute, and argument validation (for
   example reject leading wildcards in search queries). Put caps in config with safe defaults.
4. **Credentials**: one scoped, read-only credential per tool, loaded from the secrets
   interface, never from the agent or prompt.
5. **Output**: return data, not instructions. Wrap free text from logs as quoted strings.
   Mark the provenance (`source`, `query`, `time_range`) so the envelope records it.
6. **Tests** (mock the upstream with a local HTTP server, e.g. `wiremock`):
   - happy path returns the expected shape
   - each role gets the expected policy decision (one test per role)
   - over-cap request is refused or truncated with a flag
   - upstream 429/5xx/timeout handled without panics
   - a log line containing an injection string comes back as data and does not change policy
7. **Stub mode**: keep the old stub only behind `ATTEST_DEMO_MODE=1`; production mode must
   never return a `*_stub` source. Add a test that asserts this.
8. Run `attest-verify` and record evidence.
