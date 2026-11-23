# 19 — Service authentication

**Document type:** Architecture decision record (ADR)
**Status:** Accepted
**Decision:** HS256 service JWTs with a per-service audience, and HMAC-SHA256 on collector ingest. Not mTLS.

Control-plane, orchestrator, MCP gateway, and ws-gateway accept a short-lived
HS256 JWT. The `aud` claim is the service name (`control-plane`, `orchestrator`,
`mcp-gateway`, `ws-gateway`). Tokens last two minutes. Callers mint one per
request from `ATTEST_SERVICE_AUTH_SECRET`.

Collector ingest is different because the body itself is what must be
authenticated. `POST /ingest` and `POST /ingest/s3` require
`X-Attest-Timestamp` (unix seconds) and `X-Attest-Signature`, an HMAC-SHA256
of `{timestamp}.{body}` keyed by `ATTEST_INGEST_HMAC_SECRET`. A timestamp more
than five minutes from the collector clock is rejected.

`/healthz` stays open on every service. A missing secret fails closed: every
other route returns 401. The production Helm values do not contain either
secret; inject them from a Kubernetes secret.

mTLS was not chosen. It needs a certificate authority in every developer loop
and in CI before the tests can run. The JWT and the HMAC are checked in
process, with negative tests, and the same code path runs in compose.

Browser WebSockets cannot set an `Authorization` header. The workbench mints a
service token after checking the user session and the browser passes it as
`access_token`. The long-term secret stays on the server. Rejection logs
include the path only.
