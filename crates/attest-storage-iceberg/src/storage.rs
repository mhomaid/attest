//! Iceberg `Storage` backed by `object_store` (MinIO / S3).
//!
//! iceberg-rust 0.9 only ships `file://` and `memory://`. The warm tier lives
//! on S3-compatible object storage, so we inject this factory into MemoryCatalog.

use async_trait::async_trait;
use bytes::Bytes;
use futures::TryStreamExt;
use iceberg::io::{
    FileMetadata, FileRead, FileWrite, InputFile, OutputFile, Storage, StorageConfig,
    StorageFactory,
};
use iceberg::{Error, ErrorKind, Result};
use object_store::path::Path as OsPath;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectStoreStorageFactory {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
}

impl ObjectStoreStorageFactory {
    pub fn build_store(&self) -> Result<Arc<dyn ObjectStore>> {
        let store = object_store::aws::AmazonS3Builder::new()
            .with_endpoint(&self.endpoint)
            .with_bucket_name(&self.bucket)
            .with_access_key_id(&self.access_key)
            .with_secret_access_key(&self.secret_key)
            .with_region(&self.region)
            .with_allow_http(true)
            .build()
            .map_err(|e| Error::new(ErrorKind::Unexpected, format!("s3 store: {e}")))?;
        Ok(Arc::new(store))
    }
}

#[typetag::serde]
impl StorageFactory for ObjectStoreStorageFactory {
    fn build(&self, _config: &StorageConfig) -> Result<Arc<dyn Storage>> {
        Ok(Arc::new(ObjectStoreStorage {
            factory: self.clone(),
            store: Some(self.build_store()?),
        }))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectStoreStorage {
    factory: ObjectStoreStorageFactory,
    #[serde(skip)]
    store: Option<Arc<dyn ObjectStore>>,
}

impl ObjectStoreStorage {
    fn store(&self) -> Result<Arc<dyn ObjectStore>> {
        if let Some(s) = &self.store {
            return Ok(s.clone());
        }
        self.factory.build_store()
    }

    fn key(path: &str) -> Result<OsPath> {
        let key = strip_s3_prefix(path).ok_or_else(|| {
            Error::new(
                ErrorKind::DataInvalid,
                format!("expected s3:// URI, got {path}"),
            )
        })?;
        OsPath::parse(key)
            .map_err(|e| Error::new(ErrorKind::DataInvalid, format!("object path {path}: {e}")))
    }
}

/// `s3://bucket/key` → `key` (bucket is already bound on the client).
fn strip_s3_prefix(path: &str) -> Option<&str> {
    let rest = path
        .strip_prefix("s3://")
        .or_else(|| path.strip_prefix("s3a://"))?;
    rest.split_once('/').map(|(_, key)| key)
}

fn iceberg_err(context: &str, e: impl std::fmt::Display) -> Error {
    Error::new(ErrorKind::Unexpected, format!("{context}: {e}"))
}

#[async_trait]
#[typetag::serde]
impl Storage for ObjectStoreStorage {
    async fn exists(&self, path: &str) -> Result<bool> {
        let store = self.store()?;
        let key = Self::key(path)?;
        match store.head(&key).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(e) => Err(iceberg_err("head", e)),
        }
    }

    async fn metadata(&self, path: &str) -> Result<FileMetadata> {
        let store = self.store()?;
        let key = Self::key(path)?;
        let meta = store.head(&key).await.map_err(|e| iceberg_err("head", e))?;
        Ok(FileMetadata { size: meta.size })
    }

    async fn read(&self, path: &str) -> Result<Bytes> {
        let store = self.store()?;
        let key = Self::key(path)?;
        let get = store.get(&key).await.map_err(|e| iceberg_err("get", e))?;
        get.bytes().await.map_err(|e| iceberg_err("bytes", e))
    }

    async fn reader(&self, path: &str) -> Result<Box<dyn FileRead>> {
        let data = self.read(path).await?;
        Ok(Box::new(BytesRead { data }))
    }

    async fn write(&self, path: &str, bs: Bytes) -> Result<()> {
        let store = self.store()?;
        let key = Self::key(path)?;
        store
            .put(&key, PutPayload::from_bytes(bs))
            .await
            .map_err(|e| iceberg_err("put", e))?;
        Ok(())
    }

    async fn writer(&self, path: &str) -> Result<Box<dyn FileWrite>> {
        Ok(Box::new(BufferedWrite {
            storage: self.clone(),
            path: path.to_string(),
            buffer: Vec::new(),
            closed: false,
        }))
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let store = self.store()?;
        let key = Self::key(path)?;
        match store.delete(&key).await {
            Ok(()) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(iceberg_err("delete", e)),
        }
    }

    async fn delete_prefix(&self, path: &str) -> Result<()> {
        let store = self.store()?;
        let key = Self::key(path)?;
        let mut stream = store.list(Some(&key));
        while let Some(meta) = stream
            .try_next()
            .await
            .map_err(|e| iceberg_err("list", e))?
        {
            store
                .delete(&meta.location)
                .await
                .map_err(|e| iceberg_err("delete", e))?;
        }
        Ok(())
    }

    fn new_input(&self, path: &str) -> Result<InputFile> {
        Ok(InputFile::new(Arc::new(self.clone()), path.to_string()))
    }

    fn new_output(&self, path: &str) -> Result<OutputFile> {
        Ok(OutputFile::new(Arc::new(self.clone()), path.to_string()))
    }
}

#[derive(Debug)]
struct BytesRead {
    data: Bytes,
}

#[async_trait]
impl FileRead for BytesRead {
    async fn read(&self, range: Range<u64>) -> Result<Bytes> {
        let start = range.start as usize;
        let end = range.end as usize;
        if end > self.data.len() {
            return Err(Error::new(
                ErrorKind::DataInvalid,
                format!("range {start}..{end} exceeds {}", self.data.len()),
            ));
        }
        Ok(self.data.slice(start..end))
    }
}

#[derive(Debug)]
struct BufferedWrite {
    storage: ObjectStoreStorage,
    path: String,
    buffer: Vec<u8>,
    closed: bool,
}

#[async_trait]
impl FileWrite for BufferedWrite {
    async fn write(&mut self, bs: Bytes) -> Result<()> {
        if self.closed {
            return Err(Error::new(ErrorKind::DataInvalid, "write to closed file"));
        }
        self.buffer.extend_from_slice(&bs);
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        if self.closed {
            return Err(Error::new(ErrorKind::DataInvalid, "already closed"));
        }
        self.closed = true;
        let buf = std::mem::take(&mut self.buffer);
        self.storage.write(&self.path, Bytes::from(buf)).await
    }
}

#[cfg(test)]
mod tests {
    use super::strip_s3_prefix;

    #[test]
    fn strips_bucket_from_s3_uri() {
        assert_eq!(
            strip_s3_prefix("s3://attest-warm/attest/cloudtrail/data/a.parquet"),
            Some("attest/cloudtrail/data/a.parquet")
        );
        assert_eq!(
            strip_s3_prefix("s3a://attest-warm/metadata/v1.json"),
            Some("metadata/v1.json")
        );
    }
}
