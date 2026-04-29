//! Scenario factories — each produces a synthetic `FlatEvent` matching real
//! CloudTrail shapes so RisingWave rules actually fire.

use attest_common::FlatEvent;
use chrono::Utc;
use rand::Rng;
use uuid::Uuid;

const HOME_REGION: &str = "us-east-1";

/// Stable region pool for simulated tenants.
pub const REGIONS: &[&str] = &[
    "us-east-1", "us-west-2", "eu-west-1", "ap-southeast-1", "sa-east-1",
];

/// Build a benign login event (home region, success).
pub fn benign_login(tenant_id: &str, user_name: &str) -> FlatEvent {
    FlatEvent {
        event_id: Uuid::new_v4().to_string(),
        class_uid: "3002".into(),
        time: Utc::now().to_rfc3339(),
        tenant_id: tenant_id.to_string(),
        actor_user_name: user_name.to_string(),
        actor_user_uid: format!("arn:aws:iam::{}:user/{}", random_account(), user_name),
        cloud_region: HOME_REGION.into(),
        cloud_account_uid: random_account(),
        severity: "informational".into(),
        auth_status: Some("Success".into()),
        api_operation: Some("ConsoleLogin".into()),
        api_service: Some("signin.amazonaws.com".into()),
        raw: None,
    }
}

/// Anomalous geo-login — triggers `aws_login_anomalous_geo` detection.
pub fn geo_anomaly(tenant_id: &str, user_name: &str) -> FlatEvent {
    let foreign_region = pick_foreign_region();
    FlatEvent {
        event_id: Uuid::new_v4().to_string(),
        class_uid: "3002".into(),
        time: Utc::now().to_rfc3339(),
        tenant_id: tenant_id.to_string(),
        actor_user_name: user_name.to_string(),
        actor_user_uid: format!("arn:aws:iam::{}:user/{}", random_account(), user_name),
        cloud_region: foreign_region,
        cloud_account_uid: random_account(),
        severity: "medium".into(),
        auth_status: Some("Success".into()),
        api_operation: Some("ConsoleLogin".into()),
        api_service: Some("signin.amazonaws.com".into()),
        raw: None,
    }
}

/// Rapid repeated failures — brute-force login attempt.
pub fn brute_force(tenant_id: &str, user_name: &str) -> FlatEvent {
    FlatEvent {
        event_id: Uuid::new_v4().to_string(),
        class_uid: "3002".into(),
        time: Utc::now().to_rfc3339(),
        tenant_id: tenant_id.to_string(),
        actor_user_name: user_name.to_string(),
        actor_user_uid: format!("arn:aws:iam::{}:user/{}", random_account(), user_name),
        cloud_region: HOME_REGION.into(),
        cloud_account_uid: random_account(),
        severity: "high".into(),
        auth_status: Some("Failure".into()),
        api_operation: Some("ConsoleLogin".into()),
        api_service: Some("signin.amazonaws.com".into()),
        raw: None,
    }
}

/// Root account activity — high-severity cloud event.
pub fn root_account(tenant_id: &str) -> FlatEvent {
    FlatEvent {
        event_id: Uuid::new_v4().to_string(),
        class_uid: "6003".into(),
        time: Utc::now().to_rfc3339(),
        tenant_id: tenant_id.to_string(),
        actor_user_name: "root".to_string(),
        actor_user_uid: format!("arn:aws:iam::{}:root", random_account()),
        cloud_region: HOME_REGION.into(),
        cloud_account_uid: random_account(),
        severity: "critical".into(),
        auth_status: None,
        api_operation: Some("CreateAccessKey".into()),
        api_service: Some("iam.amazonaws.com".into()),
        raw: None,
    }
}

/// S3 data exfiltration pattern.
pub fn s3_exfil(tenant_id: &str, user_name: &str) -> FlatEvent {
    FlatEvent {
        event_id: Uuid::new_v4().to_string(),
        class_uid: "6003".into(),
        time: Utc::now().to_rfc3339(),
        tenant_id: tenant_id.to_string(),
        actor_user_name: user_name.to_string(),
        actor_user_uid: format!("arn:aws:iam::{}:user/{}", random_account(), user_name),
        cloud_region: pick_foreign_region(),
        cloud_account_uid: random_account(),
        severity: "high".into(),
        auth_status: None,
        api_operation: Some("GetObject".into()),
        api_service: Some("s3.amazonaws.com".into()),
        raw: None,
    }
}

/// MFA bypass — success without MFA device.
pub fn mfa_bypass(tenant_id: &str, user_name: &str) -> FlatEvent {
    FlatEvent {
        event_id: Uuid::new_v4().to_string(),
        class_uid: "3002".into(),
        time: Utc::now().to_rfc3339(),
        tenant_id: tenant_id.to_string(),
        actor_user_name: user_name.to_string(),
        actor_user_uid: format!("arn:aws:iam::{}:user/{}", random_account(), user_name),
        cloud_region: HOME_REGION.into(),
        cloud_account_uid: random_account(),
        severity: "high".into(),
        auth_status: Some("Success".into()),
        api_operation: Some("ConsoleLogin".into()),
        api_service: Some("signin.amazonaws.com".into()),
        raw: None,
    }
}

/// Weighted scenario picker — 92% benign, 8% attacks.
pub fn pick_event(tenant_id: &str, user_name: &str) -> FlatEvent {
    let mut rng = rand::thread_rng();
    let n: u32 = rng.gen_range(0..1000);
    match n {
        0..=919 => benign_login(tenant_id, user_name),
        920..=969 => geo_anomaly(tenant_id, user_name),
        970..=989 => brute_force(tenant_id, user_name),
        990..=994 => root_account(tenant_id),
        995..=997 => s3_exfil(tenant_id, user_name),
        _ => mfa_bypass(tenant_id, user_name),
    }
}

fn random_account() -> String {
    let mut rng = rand::thread_rng();
    format!("{:012}", rng.gen_range(100_000_000_000u64..999_999_999_999u64))
}

fn pick_foreign_region() -> String {
    let mut rng = rand::thread_rng();
    let foreign: &[&str] = &["ap-northeast-1", "me-south-1", "af-south-1", "eu-north-1"];
    foreign[rng.gen_range(0..foreign.len())].to_string()
}
