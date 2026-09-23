//! Phase 8 — trace WebSocket receives `TraceStep` JSON from Kafka after `/triage`.
//!
//! Run with `ATTEST_E2E=1 ATTEST_PHASE8_LIVE=1`, orchestrator + ws-gateway + Redpanda reachable.

use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use uuid::Uuid;

#[derive(Serialize)]
struct WsClaims {
    sub: String,
    email: String,
    tenant_id: String,
    case_id: String,
    exp: usize,
}

fn phase8_live_enabled() -> bool {
    e2e_tests::e2e_enabled() && std::env::var("ATTEST_PHASE8_LIVE").is_ok()
}

fn orchestrator_url() -> String {
    std::env::var("ORCHESTRATOR_URL").unwrap_or_else(|_| "http://localhost:4300".into())
}

fn ws_gateway_http_url() -> String {
    std::env::var("WS_GATEWAY_URL").unwrap_or_else(|_| "http://127.0.0.1:4500".into())
}

fn ws_gateway_ws_url() -> String {
    std::env::var("WS_GATEWAY_WS_URL").unwrap_or_else(|_| "ws://127.0.0.1:4500".into())
}

fn jwt_secret() -> String {
    std::env::var("ATTEST_WS_JWT_SECRET").unwrap_or_else(|_| "dev-ws-jwt-secret-change-me".into())
}

fn mint_jwt(case_id: Uuid) -> String {
    let exp = chrono::Utc::now().timestamp() as usize + 900;
    let claims = WsClaims {
        sub: "phase8-e2e".into(),
        email: "e2e@attest.local".into(),
        tenant_id: "default".into(),
        case_id: case_id.to_string(),
        exp,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(jwt_secret().as_bytes()),
    )
    .expect("jwt encode")
}

async fn wait_healthz(client: &Client, base: &str, path: &str, label: &str) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if Instant::now() >= deadline {
            panic!("{label} {base}{path} not healthy within 60s");
        }
        if let Ok(resp) = client.get(format!("{base}{path}")).send().await {
            if resp.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

#[tokio::test]
async fn phase8_workbench_ws_trace_and_heartbeat() {
    if !phase8_live_enabled() {
        eprintln!(
            "SKIP phase8_workbench_ws: set ATTEST_E2E=1 ATTEST_PHASE8_LIVE=1 with live stack"
        );
        return;
    }

    let http = Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .unwrap();
    let orch = orchestrator_url();
    let ws_http = ws_gateway_http_url();
    wait_healthz(&http, &orch, "/healthz", "orchestrator").await;
    wait_healthz(&http, &ws_http, "/healthz", "ws-gateway").await;

    let case_id = Uuid::new_v4();
    let token = mint_jwt(case_id);
    let ws_url = format!(
        "{}/ws/cases/{}/trace?token={}",
        ws_gateway_ws_url().trim_end_matches('/'),
        case_id,
        token
    );

    let (ws_stream, _) = connect_async(ws_url.as_str())
        .await
        .expect("connect ws-gateway trace socket");
    let (mut write, mut read) = ws_stream.split();

    let alert = json!({
        "event_type": "login",
        "principal": "phase8-ws@corp.example",
        "source_ip": "185.220.101.5",
        "severity_score": 0.95,
        "entity_reputation_score": 0.9,
        "baseline_deviation": 8.0,
        "is_weekend": true,
        "hour_of_day": 3,
        "failed_attempts_last_hour": 20,
        "unique_ips_last_hour": 5,
        "bytes_sent": 50000,
        "bytes_recv": 200,
    });

    let orch_post = orch.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let _ = Client::new()
            .post(format!("{orch_post}/triage"))
            .json(&json!({
                "case_id": case_id.to_string(),
                "tenant_id": "default",
                "alert": alert
            }))
            .send()
            .await;
    });

    let deadline = Instant::now() + Duration::from_secs(45);
    let mut trace_ok = false;
    let mut saw_ping = false;

    while Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(2), read.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                if let Ok(v) = serde_json::from_str::<Value>(&t) {
                    if v.get("kind").and_then(|k| k.as_str()) == Some("backpressure_dropped") {
                        continue;
                    }
                    if v.get("step_kind").is_some() {
                        trace_ok = true;
                    }
                }
            }
            Ok(Some(Ok(Message::Ping(p)))) => {
                saw_ping = true;
                let _ = write.send(Message::Pong(p)).await;
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
            Ok(Some(Err(e))) => panic!("websocket error: {e}"),
            Ok(Some(Ok(_))) => {}
            Err(_) => {
                if trace_ok && saw_ping {
                    break;
                }
            }
        }
        if trace_ok && saw_ping {
            break;
        }
    }

    assert!(
        trace_ok,
        "expected ≥1 TraceStep JSON (step_kind) from ws-gateway within 45s for case {case_id}"
    );
    assert!(
        saw_ping,
        "expected ≥1 server ping (heartbeat) from ws-gateway for case {case_id}"
    );
}
