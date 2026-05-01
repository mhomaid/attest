use crate::schema::cloudtrail_arrow_schema;
use anyhow::{Context, Result};
use arrow::array::{ArrayRef, StringArray, TimestampMicrosecondArray};
use arrow::record_batch::RecordBatch;
use chrono::DateTime;
use object_store::{path::Path as OsPath, ObjectStore, ObjectStoreExt, PutPayload};
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;

/// The flat JSON shape produced by attest-collector and stored in Redpanda.
#[derive(Debug, Deserialize)]
pub struct FlatEvent {
    pub event_id: String,
    pub class_uid: Option<String>,
    pub time: Option<String>,
    pub tenant_id: Option<String>,
    pub actor_user_name: Option<String>,
    pub actor_user_uid: Option<String>,
    pub cloud_region: Option<String>,
    pub cloud_account_uid: Option<String>,
    pub severity: Option<String>,
    pub auth_status: Option<String>,
    pub api_operation: Option<String>,
    pub api_service: Option<String>,
    pub raw: Option<serde_json::Value>,
}

pub struct ParquetBatchWriter {
    store: Arc<dyn ObjectStore>,
    bucket_prefix: String,
}

impl ParquetBatchWriter {
    pub fn new(store: Arc<dyn ObjectStore>, bucket_prefix: impl Into<String>) -> Self {
        Self {
            store,
            bucket_prefix: bucket_prefix.into(),
        }
    }

    /// Convert a slice of `FlatEvent`s into a Parquet file and write it to object storage.
    /// Returns the object path that was written.
    pub async fn write_batch(&self, events: &[FlatEvent]) -> Result<String> {
        let schema = cloudtrail_arrow_schema();
        let batch = events_to_record_batch(events, schema.clone())?;

        let mut buf: Vec<u8> = Vec::new();
        let props = WriterProperties::builder().build();
        let mut writer = ArrowWriter::try_new(&mut buf, schema, Some(props))?;
        writer.write(&batch)?;
        writer.close()?;

        let now = chrono::Utc::now();
        let path_str = format!(
            "{}/year={}/month={:02}/day={:02}/{}.parquet",
            self.bucket_prefix,
            now.format("%Y"),
            now.format("%m"),
            now.format("%d"),
            uuid::Uuid::new_v4(),
        );
        let path = OsPath::parse(&path_str).context("invalid object path")?;
        self.store
            .put(&path, PutPayload::from_bytes(buf.into()))
            .await?;
        info!("wrote {} events → {}", events.len(), path_str);
        Ok(path_str)
    }
}

fn events_to_record_batch(
    events: &[FlatEvent],
    schema: Arc<arrow::datatypes::Schema>,
) -> Result<RecordBatch> {
    let mut event_ids: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut class_uids: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut times: Vec<Option<i64>> = Vec::with_capacity(events.len());
    let mut tenant_ids: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut actor_user_names: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut actor_user_uids: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut cloud_regions: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut cloud_account_uids: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut severities: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut auth_statuses: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut api_operations: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut api_services: Vec<Option<&str>> = Vec::with_capacity(events.len());
    let mut raws: Vec<Option<String>> = Vec::with_capacity(events.len());

    for e in events {
        event_ids.push(Some(e.event_id.as_str()));
        class_uids.push(e.class_uid.as_deref());
        times.push(
            e.time
                .as_deref()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|dt| dt.timestamp_micros()),
        );
        tenant_ids.push(e.tenant_id.as_deref());
        actor_user_names.push(e.actor_user_name.as_deref());
        actor_user_uids.push(e.actor_user_uid.as_deref());
        cloud_regions.push(e.cloud_region.as_deref());
        cloud_account_uids.push(e.cloud_account_uid.as_deref());
        severities.push(e.severity.as_deref());
        auth_statuses.push(e.auth_status.as_deref());
        api_operations.push(e.api_operation.as_deref());
        api_services.push(e.api_service.as_deref());
        raws.push(e.raw.as_ref().map(|v| v.to_string()));
    }

    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(event_ids)),
        Arc::new(StringArray::from(class_uids)),
        Arc::new(TimestampMicrosecondArray::from(times).with_timezone("UTC")),
        Arc::new(StringArray::from(tenant_ids)),
        Arc::new(StringArray::from(actor_user_names)),
        Arc::new(StringArray::from(actor_user_uids)),
        Arc::new(StringArray::from(cloud_regions)),
        Arc::new(StringArray::from(cloud_account_uids)),
        Arc::new(StringArray::from(severities)),
        Arc::new(StringArray::from(auth_statuses)),
        Arc::new(StringArray::from(api_operations)),
        Arc::new(StringArray::from(api_services)),
        Arc::new(StringArray::from(
            raws.iter().map(|s| s.as_deref()).collect::<Vec<_>>(),
        )),
    ];

    Ok(RecordBatch::try_new(schema, columns)?)
}
