//! WebSocket gateway — `/ws/cases/{case_id}/trace?token=JWT` streams `TraceStep` JSON from `agent.trace_steps`.

use anyhow::Context;
use attest_attestation::{TraceStep, TRACE_STEPS_TOPIC};
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::ClientConfig;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    brokers: String,
    topic: String,
    jwt_secret: Arc<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WsClaims {
    sub: String,
    #[serde(default)]
    email: Option<String>,
    tenant_id: String,
    case_id: String,
    exp: usize,
}

#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let port: u16 = std::env::var("ATTEST_WS_GATEWAY_PORT")
        .or_else(|_| std::env::var("WS_GATEWAY_PORT"))
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4500);

    let brokers = std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:19092".into());
    let topic =
        std::env::var("ATTEST_TRACE_STEPS_TOPIC").unwrap_or_else(|_| TRACE_STEPS_TOPIC.into());
    let database_url = std::env::var("DATABASE_URL").ok();

    let jwt_secret = Arc::new(
        std::env::var("ATTEST_WS_JWT_SECRET")
            .context("ATTEST_WS_JWT_SECRET must be set for JWT validation")?,
    );

    ensure_trace_topic(&brokers, &topic).await;

    let state = AppState {
        brokers,
        topic,
        jwt_secret,
    };
    let topic_log = state.topic.clone();
    if let Some(database_url) = database_url {
        spawn_trace_writer(state.brokers.clone(), state.topic.clone(), database_url);
    } else {
        tracing::warn!("DATABASE_URL unset; durable trace writer disabled");
    }

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/ws/cases/{case_id}/trace", get(ws_trace))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, topic = %topic_log, "ws-gateway listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn ensure_trace_topic(brokers: &str, topic: &str) {
    use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
    use rdkafka::client::DefaultClientContext;

    let admin: AdminClient<DefaultClientContext> = match ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("socket.timeout.ms", "10000")
        .create()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("ws-gateway trace topic admin: {e}");
            return;
        }
    };
    let new_topic_topic = NewTopic::new(topic, 1, TopicReplication::Fixed(1));
    let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(10)));
    if let Err(e) = admin.create_topics(&[new_topic_topic], &opts).await {
        tracing::warn!("ws-gateway create_topics: {e}");
    }
}

fn spawn_trace_writer(brokers: String, topic: String, database_url: String) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = run_trace_writer(&brokers, &topic, &database_url).await {
                tracing::error!(error = %e, "durable trace writer stopped; retrying");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    });
}

async fn run_trace_writer(brokers: &str, topic: &str, database_url: &str) -> anyhow::Result<()> {
    let (pg, connection) = tokio_postgres::connect(database_url, tokio_postgres::NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!(error = %e, "postgres trace writer connection failed");
        }
    });
    ensure_trace_tables(&pg).await?;
    let group_id = std::env::var("ATTEST_TRACE_WRITER_GROUP_ID")
        .unwrap_or_else(|_| "attest-trace-writer".into());
    let offset_reset = std::env::var("ATTEST_TRACE_WRITER_OFFSET_RESET")
        .unwrap_or_else(|_| "latest".into());

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("enable.auto.commit", "false")
        .set("group.id", &group_id)
        .set("auto.offset.reset", &offset_reset)
        .create()?;
    consumer.subscribe(&[topic])?;
    tracing::info!(
        topic = %topic,
        group_id = %group_id,
        offset_reset = %offset_reset,
        "durable trace writer started"
    );

    loop {
        let msg = consumer.recv().await?;
        let Some(payload) = msg.payload() else {
            continue;
        };
        let Ok(step) = serde_json::from_slice::<TraceStep>(payload) else {
            tracing::warn!("trace writer skipped invalid trace payload");
            continue;
        };
        match insert_trace_step(&pg, &step).await {
            Ok(()) => {
                if let Err(e) = consumer.commit_message(&msg, CommitMode::Async) {
                    tracing::debug!(error = %e, "trace writer offset commit failed");
                }
            }
            Err(e) => tracing::warn!(
                error = %e,
                case_id = %step.case_id,
                "trace writer insert failed"
            ),
        }
    }
}

async fn ensure_trace_tables(pg: &tokio_postgres::Client) -> anyhow::Result<()> {
    pg.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS workbench_cases (
          case_id text PRIMARY KEY,
          tenant_id text NOT NULL DEFAULT 'default',
          event jsonb NOT NULL DEFAULT '{}'::jsonb,
          baseline jsonb,
          detection jsonb,
          triage_status text NOT NULL DEFAULT 'idle',
          triage_started_at timestamptz,
          triage_completed_at timestamptz,
          verdict jsonb,
          error text,
          created_at timestamptz NOT NULL DEFAULT now(),
          updated_at timestamptz NOT NULL DEFAULT now()
        );

        CREATE TABLE IF NOT EXISTS workbench_case_trace_steps (
          id text PRIMARY KEY,
          case_id text NOT NULL REFERENCES workbench_cases(case_id) ON DELETE CASCADE,
          agent_action_id text NOT NULL,
          agent_id text NOT NULL,
          execution_path text NOT NULL,
          step_kind text NOT NULL,
          summary text NOT NULL,
          ts timestamptz NOT NULL,
          created_at timestamptz NOT NULL DEFAULT now(),
          UNIQUE (case_id, agent_action_id, step_kind, ts)
        );

        CREATE INDEX IF NOT EXISTS workbench_case_trace_steps_case_ts_idx
          ON workbench_case_trace_steps (case_id, ts);
        ",
    )
    .await?;
    Ok(())
}

async fn insert_trace_step(pg: &tokio_postgres::Client, step: &TraceStep) -> anyhow::Result<()> {
    let case_id = step.case_id.to_string();
    let action_id = step.agent_action_id.to_string();
    let ts_millis = step.ts.timestamp_millis() as f64;
    let ts_id = step.ts.to_rfc3339();
    let id = format!("{case_id}:{action_id}:{}:{ts_id}", step.step_kind);
    let tenant_id = step.tenant_id.as_str();
    let agent_id = step.agent_id.replace('\0', "");
    let execution_path = step.execution_path.replace('\0', "");
    let step_kind = step.step_kind.replace('\0', "");
    let summary = step.summary.replace('\0', "");

    pg.execute(
        "
        INSERT INTO workbench_cases (case_id, tenant_id, event, updated_at)
        VALUES ($1, $2, '{}'::jsonb, now())
        ON CONFLICT (case_id) DO UPDATE
          SET tenant_id = EXCLUDED.tenant_id,
              updated_at = now()
        ",
        &[&case_id, &tenant_id],
    )
    .await?;

    pg.execute(
        "
        INSERT INTO workbench_case_trace_steps (
          id, case_id, agent_action_id, agent_id, execution_path, step_kind, summary, ts
        )
        VALUES (
          $1::text,
          $2::text,
          $3::text,
          $4::text,
          $5::text,
          $6::text,
          $7::text,
          to_timestamp($8::double precision / 1000.0)
        )
        ON CONFLICT (id) DO NOTHING
        ",
        &[
            &id,
            &case_id,
            &action_id,
            &agent_id,
            &execution_path,
            &step_kind,
            &summary,
            &ts_millis,
        ],
    )
    .await?;
    Ok(())
}

fn build_consumer(brokers: &str, topic: &str) -> anyhow::Result<StreamConsumer> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("enable.auto.commit", "true")
        .set("group.id", "attest-ws-gateway-trace")
        .set("auto.offset.reset", "latest")
        .create()?;
    consumer.subscribe(&[topic])?;
    Ok(consumer)
}

fn verify_token(token: &str, secret: &str, path_case_id: Uuid) -> Result<(String, String), String> {
    let mut val = Validation::new(Algorithm::HS256);
    val.validate_exp = true;
    let token_data = decode::<WsClaims>(token, &DecodingKey::from_secret(secret.as_bytes()), &val)
        .map_err(|e| e.to_string())?;

    let claims = token_data.claims;
    let claim_case = Uuid::parse_str(&claims.case_id).map_err(|e| e.to_string())?;
    if claim_case != path_case_id {
        return Err("token case_id does not match path".into());
    }
    Ok((claims.tenant_id, claims.sub))
}

async fn ws_trace(
    Path(case_id): Path<Uuid>,
    Query(q): Query<WsQuery>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let secret = state.jwt_secret.as_str();
    let (tenant_id, _sub) = match verify_token(&q.token, secret, case_id) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "JWT rejected for trace WS");
            return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
    };

    let brokers = state.brokers.clone();
    let topic = state.topic.clone();

    ws.on_upgrade(move |socket| handle_socket(socket, case_id, tenant_id, brokers, topic))
}

async fn handle_socket(
    mut socket: WebSocket,
    case_id: Uuid,
    tenant_id: String,
    brokers: String,
    topic: String,
) {
    let (tx, mut rx) = mpsc::channel::<String>(256);

    let case_filter = case_id;
    let tenant_filter = tenant_id;
    tokio::spawn(async move {
        let consumer = match build_consumer(&brokers, &topic) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "kafka consumer failed");
                return;
            }
        };

        loop {
            match consumer.recv().await {
                Ok(msg) => {
                    let Some(payload) = msg.payload() else {
                        continue;
                    };
                    let Ok(step) = serde_json::from_slice::<TraceStep>(payload) else {
                        continue;
                    };
                    if step.case_id != case_filter || step.tenant_id != tenant_filter {
                        continue;
                    }
                    let text = match std::str::from_utf8(payload) {
                        Ok(s) => s.to_string(),
                        Err(_) => continue,
                    };
                    match tx.try_send(text) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            let notice = r#"{"kind":"backpressure_dropped","n":1}"#.to_string();
                            let _ = tx.try_send(notice);
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => break,
                    }
                }
                Err(e) => tracing::debug!(error = ?e, "kafka recv"),
            }
        }
    });

    let mut ping = tokio::time::interval(Duration::from_secs(15));
    loop {
        tokio::select! {
            _ = ping.tick() => {
                if socket.send(WsMessage::Ping(Default::default())).await.is_err() {
                    break;
                }
            }
            maybe = rx.recv() => {
                let Some(line) = maybe else { break };
                if socket.send(WsMessage::Text(line.into())).await.is_err() {
                    break;
                }
            }
            inc = socket.recv() => {
                match inc {
                    Some(Ok(WsMessage::Pong(_))) | Some(Ok(WsMessage::Ping(_))) => {}
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}
