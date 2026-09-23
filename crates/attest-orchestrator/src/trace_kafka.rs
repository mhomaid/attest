//! Produce `agent.trace_steps` messages for the workbench WebSocket gateway.

use attest_attestation::{ExecutionPathKind, TraceStep, TRACE_STEPS_TOPIC};
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Context for streaming intermediate LLM steps to Kafka.
#[derive(Clone)]
pub struct TraceEmit {
    pub publisher: TracePublisher,
    pub case_id: Uuid,
    pub tenant_id: String,
    pub agent_action_id: Uuid,
    pub agent_id: String,
    pub execution_path: ExecutionPathKind,
}

impl TraceEmit {
    pub fn step(&self, step_kind: &str, summary: impl Into<String>) {
        self.publisher.publish(TraceStep::new(
            self.case_id,
            &self.tenant_id,
            self.agent_action_id,
            self.agent_id.clone(),
            &self.execution_path,
            step_kind,
            summary,
        ));
    }
}

/// Fire-and-forget Kafka publisher; no-op when `KAFKA_BROKERS` is unset or invalid.
#[derive(Clone)]
pub struct TracePublisher {
    producer: Option<Arc<FutureProducer>>,
    topic: String,
}

impl TracePublisher {
    pub fn from_env() -> Self {
        let topic =
            std::env::var("ATTEST_TRACE_STEPS_TOPIC").unwrap_or_else(|_| TRACE_STEPS_TOPIC.into());
        let producer = std::env::var("KAFKA_BROKERS").ok().and_then(|brokers| {
            match ClientConfig::new()
                .set("bootstrap.servers", &brokers)
                .set("message.timeout.ms", "5000")
                .create::<FutureProducer>()
            {
                Ok(p) => Some(Arc::new(p)),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "TracePublisher: FutureProducer create failed — trace steps disabled"
                    );
                    None
                }
            }
        });
        if producer.is_some() {
            tracing::info!(topic = %topic, "TracePublisher: Kafka trace steps enabled");
        } else {
            tracing::info!("TracePublisher: KAFKA_BROKERS unset or invalid — trace steps disabled");
        }
        Self { producer, topic }
    }

    /// Non-blocking send; logs errors at debug level.
    pub fn publish(&self, step: TraceStep) {
        let Some(prod) = self.producer.as_ref() else {
            return;
        };
        let key = step.case_id.to_string();
        let Ok(payload) = serde_json::to_string(&step) else {
            return;
        };
        let prod = prod.clone();
        let topic = self.topic.clone();
        tokio::spawn(async move {
            let res = prod
                .send(
                    FutureRecord::to(topic.as_str())
                        .key(&key)
                        .payload(payload.as_bytes()),
                    Duration::from_secs(5),
                )
                .await;
            if let Err((e, _)) = res {
                tracing::debug!(error = %e, "trace step kafka send failed");
            }
        });
    }

    /// Bind this publisher to one agent action; emit steps with [`TraceEmit::step`].
    pub fn scoped(
        &self,
        case_id: Uuid,
        tenant_id: impl Into<String>,
        agent_action_id: Uuid,
        agent_id: impl Into<String>,
        execution_path: ExecutionPathKind,
    ) -> TraceEmit {
        TraceEmit {
            publisher: self.clone(),
            case_id,
            tenant_id: tenant_id.into(),
            agent_action_id,
            agent_id: agent_id.into(),
            execution_path,
        }
    }
}

/// Ensure the trace-steps topic exists (best-effort).
pub async fn ensure_trace_topic(brokers: &str) {
    use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
    use rdkafka::client::DefaultClientContext;

    let admin: AdminClient<DefaultClientContext> = match ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("socket.timeout.ms", "10000")
        .create()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("trace topic admin client: {e}");
            return;
        }
    };
    let new_topic = NewTopic::new(TRACE_STEPS_TOPIC, 1, TopicReplication::Fixed(1));
    let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(10)));
    match admin.create_topics(&[new_topic], &opts).await {
        Ok(results) => {
            for res in results {
                match res {
                    Ok(name) => tracing::info!("kafka topic '{name}' created"),
                    Err((name, rdkafka::error::RDKafkaErrorCode::TopicAlreadyExists)) => {
                        tracing::debug!("kafka topic '{name}' already exists");
                    }
                    Err((name, e)) => tracing::warn!("kafka topic '{name}' error: {e}"),
                }
            }
        }
        Err(e) => tracing::warn!("kafka create_topics (trace steps) failed: {e}"),
    }
}
