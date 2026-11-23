use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{SecretError, SecretSource, SecretValue};

/// Wraps a source with a short TTL so remote backends (AWS) are not called per request.
/// Rotation is visible after at most `ttl`. Only successes are cached.
pub struct CachedSource {
    inner: Arc<dyn SecretSource>,
    ttl: Duration,
    entries: Mutex<HashMap<String, (Instant, SecretValue)>>,
}

impl CachedSource {
    pub fn new(inner: Arc<dyn SecretSource>, ttl: Duration) -> Self {
        Self {
            inner,
            ttl,
            entries: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl SecretSource for CachedSource {
    async fn get(&self, name: &str) -> Result<SecretValue, SecretError> {
        {
            let entries = self.entries.lock().await;
            if let Some((at, v)) = entries.get(name) {
                if at.elapsed() < self.ttl {
                    return Ok(v.clone());
                }
            }
        }
        let fresh = self.inner.get(name).await?;
        self.entries
            .lock()
            .await
            .insert(name.to_string(), (Instant::now(), fresh.clone()));
        Ok(fresh)
    }

    fn kind(&self) -> &'static str {
        self.inner.kind()
    }
}
