use crate::catalog::{persist_hint, OpenTable};
use crate::schema::cloudtrail_arrow_schema;
use anyhow::{Context, Result};
use arrow::array::{ArrayRef, StringArray, TimestampMicrosecondArray};
use arrow::record_batch::RecordBatch;
use bytes::Bytes;
use chrono::DateTime;
use iceberg::memory::MemoryCatalog;
use iceberg::spec::{DataContentType, DataFile, DataFileBuilder, DataFileFormat, Struct};
use iceberg::table::Table;
use iceberg::transaction::{ApplyTransactionAction, Transaction};
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

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

/// Result of committing a Parquet data file as an Iceberg snapshot.
#[derive(Debug, Clone)]
pub struct CommitResult {
    pub data_path: String,
    pub snapshot_id: i64,
    pub records: usize,
}

pub struct IcebergBatchWriter {
    catalog: MemoryCatalog,
    table: Mutex<Table>,
}

impl IcebergBatchWriter {
    pub fn from_open(open: OpenTable) -> Self {
        Self {
            catalog: open.catalog,
            table: Mutex::new(open.table),
        }
    }

    /// Convert events to Parquet, write the file through Iceberg FileIO, and
    /// fast-append a snapshot. Returns the data-file path and snapshot id.
    pub async fn write_batch(&self, events: &[FlatEvent]) -> Result<CommitResult> {
        if events.is_empty() {
            anyhow::bail!("refusing to commit an empty Iceberg snapshot");
        }

        let parquet = events_to_parquet(events)?;
        let file_size = parquet.len() as u64;
        let records = events.len();

        let mut table = self.table.lock().await;
        let data_path = format!(
            "{}/data/{}.parquet",
            table.metadata().location(),
            Uuid::new_v4()
        );

        table
            .file_io()
            .new_output(&data_path)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .write(Bytes::from(parquet))
            .await
            .map_err(|e| anyhow::anyhow!("write parquet: {e}"))?;

        let data_file = data_file_for(&table, &data_path, file_size, records as u64)?;
        let tx = Transaction::new(&table);
        let tx = tx
            .fast_append()
            .add_data_files(vec![data_file])
            .apply(tx)
            .map_err(|e| anyhow::anyhow!("fast_append: {e}"))?;
        let updated = tx
            .commit(&self.catalog)
            .await
            .map_err(|e| anyhow::anyhow!("iceberg commit: {e}"))?;
        persist_hint(&updated).await?;

        let snapshot_id = updated
            .metadata()
            .current_snapshot()
            .map(|s| s.snapshot_id())
            .context("commit produced no current snapshot")?;
        *table = updated;

        info!(
            records,
            snapshot_id,
            path = %data_path,
            "committed Iceberg snapshot"
        );
        Ok(CommitResult {
            data_path,
            snapshot_id,
            records,
        })
    }

    #[allow(dead_code)]
    pub async fn snapshot_count(&self) -> usize {
        self.table.lock().await.metadata().snapshots().count()
    }
}

fn data_file_for(table: &Table, path: &str, file_size: u64, records: u64) -> Result<DataFile> {
    DataFileBuilder::default()
        .content(DataContentType::Data)
        .file_path(path.to_string())
        .file_format(DataFileFormat::Parquet)
        .file_size_in_bytes(file_size)
        .record_count(records)
        .partition_spec_id(table.metadata().default_partition_spec_id())
        .partition(Struct::empty())
        .build()
        .map_err(|e| anyhow::anyhow!("data file: {e}"))
}

fn events_to_parquet(events: &[FlatEvent]) -> Result<Vec<u8>> {
    let schema = cloudtrail_arrow_schema();
    let batch = events_to_record_batch(events, schema.clone())?;
    let mut buf: Vec<u8> = Vec::new();
    let props = WriterProperties::builder().build();
    let mut writer = ArrowWriter::try_new(&mut buf, schema, Some(props))?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(buf)
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
