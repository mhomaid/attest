//! Fetch CloudTrail files from S3-compatible storage (MinIO locally, AWS in prod).
//!
//! Accepts the S3 event-notification shape CloudTrail / EventBridge emit when a
//! new log object lands, or an explicit `{ "bucket", "key" }` body.

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use flate2::read::GzDecoder;
use object_store::aws::AmazonS3Builder;
use object_store::path::Path as ObjPath;
use object_store::{ObjectStore, ObjectStoreExt};
use serde::Deserialize;
use std::io::Read;

/// CloudTrail files are typically a few MB; this caps a gzip bomb.
const MAX_DECOMPRESSED_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct S3Pointer {
    pub bucket: String,
    pub key: String,
}

/// True when the JSON looks like an AWS S3 / EventBridge object-created event,
/// not a CloudTrail `Records` envelope.
pub fn is_s3_notification(body: &serde_json::Value) -> bool {
    body.get("Records")
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("s3"))
        .is_some()
        || body.get("detail").and_then(|d| d.get("bucket")).is_some()
}

pub fn pointers_from_notification(body: &serde_json::Value) -> Vec<S3Pointer> {
    let mut out = Vec::new();
    if let Some(records) = body.get("Records").and_then(|r| r.as_array()) {
        for rec in records {
            if let (Some(bucket), Some(key)) = (
                rec.pointer("/s3/bucket/name").and_then(|v| v.as_str()),
                rec.pointer("/s3/object/key").and_then(|v| v.as_str()),
            ) {
                out.push(S3Pointer {
                    bucket: bucket.to_string(),
                    key: decode_s3_event_key(key),
                });
            }
        }
    }
    if let (Some(bucket), Some(key)) = (
        body.pointer("/detail/bucket/name").and_then(|v| v.as_str()),
        body.pointer("/detail/object/key").and_then(|v| v.as_str()),
    ) {
        out.push(S3Pointer {
            bucket: bucket.to_string(),
            key: key.to_string(),
        });
    }
    if let (Some(bucket), Some(key)) = (
        body.get("bucket").and_then(|v| v.as_str()),
        body.get("key").and_then(|v| v.as_str()),
    ) {
        out.push(S3Pointer {
            bucket: bucket.to_string(),
            key: key.to_string(),
        });
    }
    out
}

/// S3 event notifications URL-encode object keys with `+` for spaces.
fn decode_s3_event_key(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Buckets the collector may read, from `COLLECTOR_S3_ALLOWED_BUCKETS` (comma-separated).
/// Unset means S3 ingest is disabled, so a caller cannot point the collector's
/// credentials at an arbitrary bucket.
pub fn check_bucket_allowed(bucket: &str) -> Result<()> {
    let allowed = std::env::var("COLLECTOR_S3_ALLOWED_BUCKETS").unwrap_or_default();
    if allowed
        .split(',')
        .map(str::trim)
        .any(|b| !b.is_empty() && b == bucket)
    {
        return Ok(());
    }
    bail!("bucket `{bucket}` is not in COLLECTOR_S3_ALLOWED_BUCKETS")
}

/// Build an S3 client from `AWS_*` env vars. `AWS_ENDPOINT_URL` points at MinIO.
pub fn store_for_bucket(bucket: &str) -> Result<Box<dyn ObjectStore>> {
    let mut builder = AmazonS3Builder::from_env()
        .with_bucket_name(bucket)
        .with_region(std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".into()));
    if let Ok(endpoint) = std::env::var("AWS_ENDPOINT_URL") {
        builder = builder.with_endpoint(endpoint).with_allow_http(true);
    }
    Ok(Box::new(builder.build().context("S3 client")?))
}

pub async fn get_object(bucket: &str, key: &str) -> Result<Bytes> {
    check_bucket_allowed(bucket)?;
    let store = store_for_bucket(bucket)?;
    let path = ObjPath::from(key.trim_start_matches('/'));
    let get = store
        .get(&path)
        .await
        .with_context(|| format!("s3://{bucket}/{key}"))?;
    if get.meta.size > MAX_DECOMPRESSED_BYTES {
        bail!(
            "s3://{bucket}/{key} is {} bytes; limit is {MAX_DECOMPRESSED_BYTES}",
            get.meta.size
        );
    }
    get.bytes().await.context("read object body")
}

/// CloudTrail objects on S3 are usually gzip JSON.
pub fn decode_cloudtrail_bytes(key: &str, bytes: Bytes) -> Result<serde_json::Value> {
    let raw = if key.ends_with(".gz") || bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        GzDecoder::new(bytes.as_ref())
            .take(MAX_DECOMPRESSED_BYTES + 1)
            .read_to_end(&mut out)
            .context("gunzip CloudTrail object")?;
        if out.len() as u64 > MAX_DECOMPRESSED_BYTES {
            bail!("decompressed CloudTrail object exceeds {MAX_DECOMPRESSED_BYTES} bytes");
        }
        out
    } else {
        bytes.to_vec()
    };
    serde_json::from_slice(&raw).context("CloudTrail JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    #[test]
    fn detects_s3_notification() {
        let body = serde_json::json!({
            "Records": [{
                "eventSource": "aws:s3",
                "s3": {
                    "bucket": { "name": "aws-cloudtrail-logs" },
                    "object": { "key": "AWSLogs/111/CloudTrail/ap-southeast-1/file.json.gz" }
                }
            }]
        });
        assert!(is_s3_notification(&body));
        let p = pointers_from_notification(&body);
        assert_eq!(p[0].bucket, "aws-cloudtrail-logs");
        assert!(p[0].key.ends_with("file.json.gz"));
    }

    #[test]
    fn cloudtrail_records_are_not_notifications() {
        let body = serde_json::json!({
            "Records": [{ "eventName": "ConsoleLogin", "awsRegion": "us-east-1" }]
        });
        assert!(!is_s3_notification(&body));
    }

    #[test]
    fn decodes_event_keys() {
        assert_eq!(decode_s3_event_key("a+b%3Dc.json"), "a b=c.json");
        assert_eq!(decode_s3_event_key("100%"), "100%");
        assert_eq!(decode_s3_event_key("%zz%é"), "%zz%é");
    }

    #[test]
    fn decodes_plain_and_gzip_json() {
        let v =
            decode_cloudtrail_bytes("log.json", Bytes::from_static(br#"{"Records":[]}"#)).unwrap();
        assert!(v.get("Records").is_some());

        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(br#"{"Records":[{"eventName":"ConsoleLogin"}]}"#)
            .unwrap();
        let gz = Bytes::from(enc.finish().unwrap());
        let v = decode_cloudtrail_bytes("x.json.gz", gz).unwrap();
        assert_eq!(v["Records"][0]["eventName"], "ConsoleLogin");
    }

    #[test]
    fn bucket_allowlist_denies_by_default() {
        std::env::remove_var("COLLECTOR_S3_ALLOWED_BUCKETS");
        assert!(check_bucket_allowed("anything").is_err());
        std::env::set_var("COLLECTOR_S3_ALLOWED_BUCKETS", "attest-cloudtrail, other");
        assert!(check_bucket_allowed("attest-cloudtrail").is_ok());
        assert!(check_bucket_allowed("attest-attestations").is_err());
        std::env::remove_var("COLLECTOR_S3_ALLOWED_BUCKETS");
    }
}
