pub mod ast;
pub mod compiler;
pub mod error;
pub mod parser;
pub mod sigma;

pub use ast::Detection;
pub use compiler::{compile_to_risingwave, drop_view_sql};
pub use error::HeliqlError;
pub use parser::parse;

#[cfg(test)]
mod tests {
    use super::*;

    const GEO_RULE: &str = r#"
detection: aws_console_login_from_anomalous_geolocation
description: "Detects AWS console login from a region the user has not seen in 90d"
applies_to: ocsf.authentication
where:
  - event.auth_status = "Success"
entity: identity.user
condition:
  - event.cloud_region NOT IN baseline(identity.user, 90d)
severity: medium
mitre: [T1078.004]
runtime: stream
"#;

    #[test]
    fn parse_geo_detection() {
        let detections = parse(GEO_RULE).expect("parse failed");
        assert_eq!(detections.len(), 1);
        let d = &detections[0];
        assert_eq!(d.id, "aws_console_login_from_anomalous_geolocation");
        assert_eq!(d.severity, ast::Severity::Medium);
        assert_eq!(d.mitre, vec!["T1078.004"]);
        assert_eq!(d.runtime, vec![ast::Runtime::Stream]);
    }

    #[test]
    fn compile_geo_detection_produces_valid_sql() {
        let detections = parse(GEO_RULE).expect("parse failed");
        let sql = compile_to_risingwave(&detections[0]).expect("compile failed");
        assert!(sql.contains("CREATE MATERIALIZED VIEW IF NOT EXISTS det_aws_console_login_from_anomalous_geolocation"));
        assert!(sql.contains("cloudtrail_events"));
        assert!(sql.contains("auth_status"));
    }

    #[test]
    fn sigma_import_round_trip() {
        let sigma_yaml = r#"
title: AWS Root Account Login
description: Detects usage of the root account
level: high
tags:
  - attack.T1078
logsource:
  product: aws
  service: cloudtrail
detection:
  selection:
    userIdentity.type: Root
    eventName: ConsoleLogin
  condition: selection
"#;
        let det = sigma::sigma_to_detection(sigma_yaml, Some("aws_root_account_use"))
            .expect("sigma conversion failed");
        assert_eq!(det.id, "aws_root_account_use");
        assert_eq!(det.severity, ast::Severity::High);
        assert!(det.mitre.contains(&"T1078".to_string()));
    }
}
