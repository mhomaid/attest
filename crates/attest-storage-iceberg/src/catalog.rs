//! Open (or create) the `attest.cloudtrail` Iceberg table.
//!
//! Uses iceberg-rust's in-process `MemoryCatalog` with either local-fs or
//! S3/MinIO storage. A `version-hint.text` pointer is written next to the
//! metadata so a restarted writer can `register_table` instead of creating a
//! second empty table.

use crate::schema::cloudtrail_iceberg_schema;
use crate::storage::ObjectStoreStorageFactory;
use anyhow::{Context, Result};
use iceberg::io::{FileIOBuilder, LocalFsStorageFactory};
use iceberg::memory::{MemoryCatalog, MemoryCatalogBuilder, MEMORY_CATALOG_WAREHOUSE};
use iceberg::table::Table;
use iceberg::{Catalog, CatalogBuilder, NamespaceIdent, TableCreation, TableIdent};
use std::collections::HashMap;
use std::sync::Arc;

const NS: &str = "attest";
const HINT: &str = "version-hint.text";

pub struct OpenTable {
    pub catalog: MemoryCatalog,
    pub table: Table,
}

pub struct Warehouse {
    pub location: String,
    pub factory: Arc<dyn iceberg::io::StorageFactory>,
}

pub struct S3Settings {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
}

impl Warehouse {
    pub fn local_fs(path: impl AsRef<std::path::Path>) -> Result<Self> {
        if !path.as_ref().exists() {
            std::fs::create_dir_all(path.as_ref())?;
        }
        let abs = std::fs::canonicalize(path.as_ref())?;
        let location = format!("file://{}", abs.display());
        Ok(Self {
            location,
            factory: Arc::new(LocalFsStorageFactory),
        })
    }

    pub fn s3(settings: S3Settings, prefix: &str) -> Self {
        let location = format!("s3://{}/{}", settings.bucket, prefix.trim_matches('/'));
        Self {
            location,
            factory: Arc::new(ObjectStoreStorageFactory {
                endpoint: settings.endpoint,
                bucket: settings.bucket,
                access_key: settings.access_key,
                secret_key: settings.secret_key,
                region: settings.region,
            }),
        }
    }
}

pub async fn open_or_create_table(warehouse: Warehouse, table_name: &str) -> Result<OpenTable> {
    let file_io = FileIOBuilder::new(warehouse.factory.clone()).build();
    let catalog = MemoryCatalogBuilder::default()
        .with_storage_factory(warehouse.factory)
        .load(
            "attest",
            HashMap::from([(
                MEMORY_CATALOG_WAREHOUSE.to_string(),
                warehouse.location.clone(),
            )]),
        )
        .await
        .map_err(|e| anyhow::anyhow!("catalog load: {e}"))?;

    let ns = NamespaceIdent::new(NS.to_string());
    if !catalog
        .namespace_exists(&ns)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?
    {
        catalog
            .create_namespace(&ns, HashMap::new())
            .await
            .map_err(|e| anyhow::anyhow!("create namespace: {e}"))?;
    }

    let ident = TableIdent::new(ns.clone(), table_name.to_string());
    let table_location = format!(
        "{}/{NS}/{table_name}",
        warehouse.location.trim_end_matches('/')
    );
    let hint_path = format!("{table_location}/metadata/{HINT}");

    let table = if file_io
        .exists(&hint_path)
        .await
        .map_err(|e| anyhow::anyhow!("hint exists: {e}"))?
    {
        let loc = String::from_utf8(
            file_io
                .new_input(&hint_path)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .read()
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .to_vec(),
        )
        .context("version-hint is not utf8")?
        .trim()
        .to_string();
        catalog
            .register_table(&ident, loc)
            .await
            .map_err(|e| anyhow::anyhow!("register_table: {e}"))?
    } else {
        let creation = TableCreation::builder()
            .name(table_name.to_string())
            .schema(cloudtrail_iceberg_schema()?)
            .location(table_location)
            .build();
        catalog
            .create_table(&ns, creation)
            .await
            .map_err(|e| anyhow::anyhow!("create_table: {e}"))?
    };

    persist_hint(&table).await?;
    Ok(OpenTable { catalog, table })
}

pub async fn persist_hint(table: &Table) -> Result<()> {
    let loc = table
        .metadata_location()
        .ok_or_else(|| anyhow::anyhow!("table has no metadata location"))?;
    let table_loc = table.metadata().location();
    let hint = format!("{table_loc}/metadata/{HINT}");
    table
        .file_io()
        .new_output(&hint)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .write(bytes::Bytes::from(loc.as_bytes().to_vec()))
        .await
        .map_err(|e| anyhow::anyhow!("write version-hint: {e}"))?;
    Ok(())
}
