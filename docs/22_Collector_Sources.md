# Collector sources

The collector ingests through a `Source` trait (`id`, `decode`, `normalize`). CloudTrail is the
first source. A new vendor implements the trait, gets a fixture test, and is registered on the
ingest path. See `.cursor/skills/attest-add-source/SKILL.md`.

## Topics

`ATTEST_TOPIC_PREFIX` defaults to `attest`.

| Stream | Topic |
| --- | --- |
| One source's normalized events | `<prefix>.raw.<source id>` |
| Sources that are already alerts | `<prefix>.alerts` as well |

CloudTrail's id is `cloudtrail`, so its raw topic is `attest.raw.cloudtrail`. It is not an alert
source. Until RisingWave is moved, the collector also writes the historical `cloudtrail` topic
(`ATTEST_MIRROR_LEGACY_CLOUDTRAIL` defaults to on). The RisingWave table is not recreated.
`phase1_baseline.sql` adds `http_path`, `http_status`, `src_ip`, `asn`, and `disposition` with
`ADD COLUMN IF NOT EXISTS`.

## Pointing a vendor at the collector

Send an HMAC-signed POST to `/ingest` (`X-Attest-Timestamp`, `X-Attest-Signature`), the same
check CloudTrail uses. The source id is chosen by the collector from the payload shape today
(CloudTrail records or an S3 notification). Later sources get their own path. Bodies over the
configured limit are rejected, and raw payloads are not logged at info level.
