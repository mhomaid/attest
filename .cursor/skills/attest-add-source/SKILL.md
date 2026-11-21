---
name: attest-add-source
description: Add a new log or alert source to attest-collector (for example Graylog alerts, Cloudflare firewall events, GuardDuty, Wazuh) with OCSF 1.3 normalization, authenticated ingest, redaction and synthetic fixture tests. Use when a task adds or changes an ingest path.
---

# Add a source to the collector

Today `attest-collector` is CloudTrail-only (`normalizer::normalize_cloudtrail`, topic
`cloudtrail`). Task T-020 introduces a `Source` trait. If T-020 is not `done`, do it first.

## Shape of a source

```rust
pub trait Source: Send + Sync {
    /// Stable id, used in topic names (`attest.raw.<id>`) and metrics.
    fn id(&self) -> &'static str;
    /// Decode one transport payload (HTTP body, S3 object) into raw records.
    fn decode(&self, bytes: &[u8], content_type: Option<&str>) -> Result<Vec<serde_json::Value>, CollectorError>;
    /// Map one raw record to OCSF. Unknown fields go under `unmapped.<prefix>.*`.
    fn normalize(&self, raw: &serde_json::Value, ctx: &NormalizeCtx) -> Result<OcsfEvent, CollectorError>;
}
```

`NormalizeCtx` carries the tenant ID, the configurable extension prefix and the field
mapping loaded from config (T-024). A source never hard-codes deployer field names.

## Steps

1. **Pick the OCSF class.** Alerts from another tool → Detection Finding (2004). Auth → 3002.
   API activity → 6003. HTTP / WAF → HTTP Activity (4002) with `disposition`. Write the
   choice in the PR.
2. **Fixtures first.** Build 3 to 6 synthetic records from the vendor's public schema docs
   under `crates/attest-collector/tests/fixtures/<source>/`: one normal, one attack-shaped,
   one malformed, one with extra unknown fields. No real logs.
3. **Tests** (`crates/attest-collector/tests/<source>.rs`):
   - each fixture normalizes to the expected class and key fields
   - malformed input returns an error, never a panic
   - unknown fields land under `unmapped.<prefix>`
   - redaction: configured sensitive fields (email, phone, card-like numbers) are hashed or dropped
   - ingest without a valid HMAC signature or mTLS identity returns 401 (once T-010 is done)
4. **Implement** the source, register it, route it to `attest.raw.<id>`, and to
   `attest.alerts` when it is already an alert.
5. **Config**: add env/config keys to `env.example` and Helm `values.yaml`; default disabled.
6. **Docs**: a short section in `docs/` on how to point the vendor at the collector.
7. Run `attest-verify` (`verify.sh full`) and record evidence.

## Rules

- Bounded memory: stream large S3 objects, cap batch sizes, and reject bodies over a configured limit.
- Never log full raw payloads at info level. They may hold personal data.
- Pull-style sources (polling an API) keep a cursor and back off on 429/5xx.
