use crate::writer::{FlatEvent, IcebergBatchWriter};
use anyhow::Result;
use rdkafka::{
    consumer::{CommitMode, Consumer, StreamConsumer},
    ClientConfig, Message,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, Instant};
use tracing::{error, info, warn};

pub struct IcebergConsumer {
    consumer: StreamConsumer,
    writer: Arc<IcebergBatchWriter>,
    batch_size: usize,
    flush_interval: Duration,
}

impl IcebergConsumer {
    pub fn new(
        brokers: &str,
        group_id: &str,
        topic: &str,
        writer: Arc<IcebergBatchWriter>,
        batch_size: usize,
        flush_interval_secs: u64,
    ) -> Result<Self> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("group.id", group_id)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .create()?;

        consumer.subscribe(&[topic])?;
        info!("IcebergConsumer subscribed to {topic}");

        Ok(Self {
            consumer,
            writer,
            batch_size,
            flush_interval: Duration::from_secs(flush_interval_secs),
        })
    }

    /// Run the consume → batch → Iceberg commit loop until the process is signalled.
    pub async fn run(self) -> Result<()> {
        let mut buffer: Vec<FlatEvent> = Vec::with_capacity(self.batch_size);
        let mut ticker = interval(self.flush_interval);
        let mut last_flush = Instant::now();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if !buffer.is_empty() {
                        let elapsed = last_flush.elapsed();
                        info!("flush timer: {} events after {:.1}s", buffer.len(), elapsed.as_secs_f32());
                        flush(&self.writer, &mut buffer).await;
                        last_flush = Instant::now();
                    }
                }

                msg = self.consumer.recv() => {
                    match msg {
                        Err(e) => warn!("Kafka recv error: {}", e),
                        Ok(m) => {
                            if let Some(payload) = m.payload() {
                                match serde_json::from_slice::<FlatEvent>(payload) {
                                    Ok(event) => {
                                        buffer.push(event);
                                        if buffer.len() >= self.batch_size {
                                            info!("batch full ({} events), flushing", buffer.len());
                                            flush(&self.writer, &mut buffer).await;
                                            last_flush = Instant::now();
                                        }
                                    }
                                    Err(e) => warn!("deserialize error: {}", e),
                                }
                            }
                            if let Err(e) = self.consumer.commit_message(&m, CommitMode::Async) {
                                error!("commit error: {}", e);
                            }
                        }
                    }
                }

                _ = tokio::signal::ctrl_c() => {
                    info!("shutdown signal; flushing {} remaining events", buffer.len());
                    if !buffer.is_empty() {
                        flush(&self.writer, &mut buffer).await;
                    }
                    break;
                }
            }
        }
        Ok(())
    }
}

async fn flush(writer: &IcebergBatchWriter, buffer: &mut Vec<FlatEvent>) {
    match writer.write_batch(buffer).await {
        Ok(commit) => info!(
            "flushed {} events → snapshot {} ({})",
            commit.records, commit.snapshot_id, commit.data_path
        ),
        Err(e) => error!("Iceberg flush error: {e}"),
    }
    buffer.clear();
}
