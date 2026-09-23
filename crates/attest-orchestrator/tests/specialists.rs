//! Coordinator / hunter / responder unit tests (no live LLM).

use attest_attestation::{Signer, Verdict};
use attest_orchestrator::coordinator::{route, CoordinateRequest};
use attest_orchestrator::mcp_client::McpClient;
use attest_orchestrator::shadow_check::ShadowChecker;
use attest_orchestrator::specialists::{run_hunt, run_respond, HuntRequest, RespondRequest};
use attest_policy_engine::AgentRole;
use serde_json::json;
use std::collections::HashSet;

#[test]
fn coordinator_routes_confident_tp_to_responder() {
    let d = route(CoordinateRequest {
        case_id: None,
        tenant_id: Some("demo".into()),
        verdict: Verdict::TruePositive,
        calibrated_confidence: 0.93,
        novelty_score: 0.1,
    });
    assert_eq!(d.invoke, vec![AgentRole::Responder]);
    assert_eq!(d.execution_path, "classifier");
}

#[tokio::test]
async fn hunter_fallback_signs_envelope_without_llm() {
    let dir = tempfile::tempdir().unwrap();
    let log = attest_attestation::AttestationLog::new(dir.path().join("hunt.ndjson"));
    let signer = Signer::generate();
    let mcp = McpClient::new("http://127.0.0.1:9");

    let out = run_hunt(
        HuntRequest {
            case_id: None,
            tenant_id: Some("demo".into()),
            hypothesis: "root console login from a new country".into(),
            alert: json!({"event":"ConsoleLogin"}),
        },
        None,
        &mcp,
        &signer,
        &log,
        "hunter prompt",
        "abc",
        "hunter-v1",
    )
    .await
    .expect("hunt fallback");

    assert!(out.fallback);
    assert_eq!(out.verdict, Verdict::NeedsInvestigation);
    assert!(!out.queried_warm_tier);
    let envs = log.read_all().await.expect("log");
    assert_eq!(envs.len(), 1);
    assert_eq!(envs[0].agent_id, "hunter-v1");
}

#[tokio::test]
async fn responder_blocks_protected_principal() {
    let dir = tempfile::tempdir().unwrap();
    let log = attest_attestation::AttestationLog::new(dir.path().join("resp.ndjson"));
    let signer = Signer::generate();
    let mcp = McpClient::new("http://127.0.0.1:9");
    let shadow = ShadowChecker::new(["ceo@corp.com".into()].into(), 0.90, true, HashSet::new());

    let out = run_respond(
        RespondRequest {
            case_id: None,
            tenant_id: Some("demo".into()),
            action: "idp_revoke_session".into(),
            target: "ceo@corp.com".into(),
            calibrated_confidence: 0.99,
            blast_radius: 1,
            alert: json!({}),
        },
        None,
        &mcp,
        &signer,
        &log,
        &shadow,
        "responder prompt",
        "def",
        "responder-v1",
    )
    .await
    .expect("respond");

    assert!(!out.executed);
    assert!(!out.shadow_check.allowed);
    assert!(out.shadow_check.reason.contains("do-not-touch"));
}

#[tokio::test]
async fn responder_allows_high_confidence_revoke_as_planned() {
    let dir = tempfile::tempdir().unwrap();
    let log = attest_attestation::AttestationLog::new(dir.path().join("resp2.ndjson"));
    let signer = Signer::generate();
    let mcp = McpClient::new("http://127.0.0.1:9");
    let shadow = ShadowChecker::new(HashSet::new(), 0.90, true, HashSet::new());

    let out = run_respond(
        RespondRequest {
            case_id: None,
            tenant_id: Some("demo".into()),
            action: "idp_revoke_session".into(),
            target: "alice@corp.com".into(),
            calibrated_confidence: 0.95,
            blast_radius: 1,
            alert: json!({}),
        },
        None,
        &mcp,
        &signer,
        &log,
        &shadow,
        "responder prompt",
        "def",
        "responder-v1",
    )
    .await
    .expect("respond");

    assert!(out.shadow_check.allowed, "{}", out.shadow_check.reason);
    assert!(!out.executed, "no LLM and no live IdP — planned only");
    assert!(out.fallback);
}
