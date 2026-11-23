# Deploy Attest for one organization

Prerequisites, the private overlay, configuration, a shadow rollout, go-live
gates, and rollback. Organization-specific values stay in the overlay.

## Pick up

1. Clone this repo.
2. Copy `examples/overlay/` into a private repository. That directory is the
   starting policy, field map, and env file. Replace every placeholder there.
3. `make dev-up-all`, then `make dev-up-live`.
4. Run the workbench with the private `.env`. Sign-in is the local Keycloak
   realm until you change `ATTEST_OIDC_ISSUER`.
5. Confirm one edge post and one SIEM-shaped alert arrived, then follow the
   shadow-mode steps below before any production host.

## Local live context

`make dev-up-live` starts three services and turns the collector flags on:

| Service | What it is | Where |
| --- | --- | --- |
| Keycloak 26.4, realm `example` | Identity provider | http://localhost:8180 (admin / admin). Client `attest-workbench`, secret `e2e-client-secret`. |
| Graylog 6.1 | SIEM | http://localhost:9000 (`admin` / `dev-only-graylog-admin`). Search URL `http://graylog:9000` inside Compose. |
| `edge-logpush` | Live edge and SIEM sender | Posts one firewall event and one Graylog-shaped alert to the collector with the ingest HMAC. |

Point the workbench at the identity provider with `ATTEST_OIDC_ISSUER=http://localhost:8180/realms/example`, `ATTEST_OIDC_CLIENT_ID=attest-workbench`, and `ATTEST_OIDC_CLIENT_SECRET=e2e-client-secret`. Create a Graylog API token and set `ATTEST_GRAYLOG_TOKEN` before the investigator search will succeed. The default stack does not start these.

## Prerequisites

- A Kubernetes cluster you already run. This chart does not create one.
- External Kafka, ClickHouse, Postgres, and S3-compatible object storage.
- An OIDC issuer. Production sets `ATTEST_AUTH_PASSWORD=off`.
- Rust 1.98 and Helm 3 if you render the chart locally.

## Overlay layout

Keep a private repository next to this one:

```
overlay/
  values.yaml          # hosts, secret names, topic prefix
  policy.yaml          # do-not-touch users, accounts, IPs, ASNs, tenants
  field-map.yaml       # private field names to OCSF paths
  secrets/             # never commit these
```

Mount `policy.yaml` as `ATTEST_POLICY_FILE` and `field-map.yaml` as
`ATTEST_FIELD_MAP_FILE`. An invalid policy file stops the MCP gateway at startup.

## Configuration

Keys used here also appear in `env.example` or Helm values:

- `ATTEST_SERVICE_AUTH_SECRET`, `ATTEST_INGEST_HMAC_SECRET`
- `ATTEST_TOPIC_PREFIX`, `ATTEST_MIRROR_LEGACY_CLOUDTRAIL`
- `ATTEST_SOURCE_GRAYLOG`, `ATTEST_SOURCE_EDGE`, `ATTEST_GRAYLOG_URL`
- `ATTEST_POLICY_FILE`, `ATTEST_FIELD_MAP_FILE`
- `ATTEST_AUTH_PASSWORD`, `ATTEST_OIDC_ISSUER`, `ATTEST_OIDC_CLIENT_ID`
- `ATTEST_ANCHOR_EVERY_SECS`, `ATTEST_ANCHOR_WEBHOOK`

Helm: start from `infra/helm/attest/values-production.example.yaml`. Secrets are
referenced by name. `auth.password` is `off`.

## Shadow-mode rollout

1. Deploy with Graylog and edge sources off.
2. Turn on `ATTEST_SOURCE_GRAYLOG=on` for one tenant and confirm alerts on
   `attest.alerts` while responders stay in record-only mode.
3. Compare investigator output with the SIEM before enabling any output plugin.

## Go-live gates

- `make helm-template` and `make tf-validate` pass for the overlay values.
- `attest verify` passes on a sample of signed envelopes, including `--anchors`
  once the anchor bucket is receiving objects.
- The do-not-touch list is loaded and a protected user cannot be acted on.
- Password sign-in is off.

## Rollback

Point the release back at the previous chart version. Sources are flags: set
`ATTEST_SOURCE_GRAYLOG=off` and `ATTEST_SOURCE_EDGE=off` to stop new ingest
without deleting the hash chain. Do not delete the anchor bucket to roll back.
