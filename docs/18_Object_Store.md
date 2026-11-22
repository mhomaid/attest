# 18 — Local object store: Garage

**Document type:** Architecture decision record (ADR)
**Status:** Accepted
**Decision:** Local Compose and CI use Garage as the S3-compatible object store. Production uses the cloud's object store (S3, or GCS via the S3 API).

## Context

Attest only needs the S3 API: Iceberg parquet, the attestation log, collector object reads, and RisingWave state. MinIO community edition is archived and the current official image refuses requests until a license is installed. A pinned third-party rebuild is not a store we should keep as the default.

## Decision

Garage (`dxflrs/garage:v2.5.0`) runs as a single node in Compose. `garage server --single-node --default-bucket` assigns the layout and creates `attest-warm`. The official image has no shell, so `garage-init` calls the admin API to create `attest-attestations`, `attest-cloudtrail`, and `risingwave`, and grants the dev key.

RisingWave still uses the `hummock+minio://` URL scheme. That name is RisingWave's path-style S3 backend, not a MinIO server. The host in the URL is `garage:3900`.

Clients set `AWS_ENDPOINT_URL` (or `S3_ENDPOINT`) and turn off virtual-hosted bucket URLs. Garage serves path-style requests on port 3900. The local signing region is `custom`: RisingWave's `hummock+minio://` backend always signs with that region, and ClickHouse plus the Rust S3 clients use the same value.

The hosted Railway lab still has a service that was created under the old name. That lab is not the local default. Replacing it is separate from this decision.

## Consequences

- `make dev-up-infra` and the CI integration job start `garage`, not MinIO.
- Dev credentials live in Compose and `env.example`. They are not a production key.
- A real deployment leaves `objectStore.endpoint` empty on AWS and uses the cloud endpoint elsewhere. It does not run this single-node Garage.
