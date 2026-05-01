/**
 * Simulation scenario templates.
 *
 * Each scenario provides:
 *  - metadata: id, title, description, MITRE technique, severity, source
 *  - defaultParams: pre-filled form values
 *  - buildCloudTrail(params): realistic CloudTrail Records envelope for the collector
 *  - buildAlert(params):  flat feature map for the orchestrator /triage endpoint
 *
 * Feature names match AlertFeatures::COLUMN_ORDER in attest-feature-extractor exactly.
 */

export type ScenarioParams = {
  username: string;
  source_ip: string;
  region: string;
  severity: number; // 0–1
};

export type Scenario = {
  id: string;
  title: string;
  description: string;
  mitre: string;
  mitreName: string;
  severity: "critical" | "high" | "medium" | "low";
  source: string;
  defaultParams: ScenarioParams;
  buildCloudTrail: (params: ScenarioParams) => object;
  buildAlert: (params: ScenarioParams) => object;
};

function nowIso(): string {
  return new Date().toISOString();
}

// ── Scenarios ─────────────────────────────────────────────────────────────────

export const SCENARIOS: Scenario[] = [
  {
    id: "geo_anomaly",
    title: "Console login — anomalous geography",
    description:
      "A successful AWS console login from a region the actor has never accessed. Geo-velocity and baseline deviation are the primary signals.",
    mitre: "T1078.004",
    mitreName: "Valid Accounts: Cloud Accounts",
    severity: "high",
    source: "AWS CloudTrail",
    defaultParams: {
      username: "alice@acme.com",
      source_ip: "203.0.113.42",
      region: "ap-northeast-1",
      severity: 0.8,
    },
    buildCloudTrail: (p) => ({
      Records: [
        {
          eventVersion: "1.08",
          userIdentity: {
            type: "IAMUser",
            userName: p.username,
            arn: `arn:aws:iam::123456789012:user/${p.username}`,
          },
          eventTime: nowIso(),
          eventSource: "signin.amazonaws.com",
          eventName: "ConsoleLogin",
          awsRegion: p.region,
          sourceIPAddress: p.source_ip,
          userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
          requestParameters: null,
          responseElements: { ConsoleLogin: "Success" },
          additionalEventData: {
            MobileVersion: "No",
            LoginTo: "https://console.aws.amazon.com/console/home",
            MFAUsed: "Yes",
          },
        },
      ],
    }),
    buildAlert: (p) => ({
      case_class: "geo_anomaly",
      actor_user_name: p.username,
      source_ip: p.source_ip,
      cloud_region: p.region,
      severity_score: p.severity,
      source_class_id: 3002,
      entity_reputation_score: 0.1,
      baseline_deviation: 4.2,
      threat_intel_hit_count: 0,
      hour_of_day: new Date().getHours(),
      asset_criticality: 0.8,
      prior_disposition_ratio: 0.65,
    }),
  },

  {
    id: "brute_force",
    title: "Okta brute-force authentication",
    description:
      "Repeated failed Okta authentication attempts against a single account within a short window, indicating credential stuffing or brute-force attack.",
    mitre: "T1110.001",
    mitreName: "Brute Force: Password Guessing",
    severity: "high",
    source: "Okta System Log",
    defaultParams: {
      username: "bob@acme.com",
      source_ip: "198.51.100.77",
      region: "us-east-1",
      severity: 0.75,
    },
    buildCloudTrail: (p) => ({
      Records: Array.from({ length: 6 }, (_, i) => ({
        eventVersion: "1.08",
        userIdentity: { type: "IAMUser", userName: p.username },
        eventTime: new Date(Date.now() - (5 - i) * 20_000).toISOString(),
        eventSource: "okta.com",
        eventName: "user.session.start",
        awsRegion: p.region,
        sourceIPAddress: p.source_ip,
        userAgent: "python-requests/2.28",
        responseElements: { outcome: { result: "FAILURE", reason: "INVALID_CREDENTIALS" } },
      })),
    }),
    buildAlert: (p) => ({
      case_class: "brute_force",
      actor_user_name: p.username,
      source_ip: p.source_ip,
      cloud_region: p.region,
      severity_score: p.severity,
      source_class_id: 3002,
      entity_reputation_score: 0.6,
      baseline_deviation: 2.8,
      threat_intel_hit_count: 2,
      hour_of_day: new Date().getHours(),
      asset_criticality: 0.5,
      prior_disposition_ratio: 0.72,
    }),
  },

  {
    id: "root_account",
    title: "AWS root account activity",
    description:
      "Direct use of the AWS root account — the most privileged identity. Any root activity is a critical signal regardless of legitimacy.",
    mitre: "T1078.004",
    mitreName: "Valid Accounts: Cloud Accounts",
    severity: "critical",
    source: "AWS CloudTrail",
    defaultParams: {
      username: "root",
      source_ip: "10.0.0.5",
      region: "us-east-1",
      severity: 1.0,
    },
    buildCloudTrail: (p) => ({
      Records: [
        {
          eventVersion: "1.08",
          userIdentity: {
            type: "Root",
            arn: "arn:aws:iam::123456789012:root",
            accountId: "123456789012",
          },
          eventTime: nowIso(),
          eventSource: "iam.amazonaws.com",
          eventName: "CreateAccessKey",
          awsRegion: p.region,
          sourceIPAddress: p.source_ip,
          userAgent: "aws-cli/2.15.0",
          requestParameters: { userName: p.username },
          responseElements: {
            accessKey: { accessKeyId: "AKIAIOSFODNN7EXAMPLE", status: "Active" },
          },
        },
      ],
    }),
    buildAlert: (p) => ({
      case_class: "default",
      actor_user_name: "root",
      source_ip: p.source_ip,
      cloud_region: p.region,
      severity_score: p.severity,
      source_class_id: 6003,
      entity_reputation_score: 0.0,
      baseline_deviation: 6.0,
      threat_intel_hit_count: 0,
      hour_of_day: new Date().getHours(),
      asset_criticality: 1.0,
      prior_disposition_ratio: 0.9,
    }),
  },

  {
    id: "s3_exfil",
    title: "S3 bucket policy made public",
    description:
      "An IAM user changed an S3 bucket ACL or policy to grant public read access, potentially exposing sensitive data to the internet.",
    mitre: "T1530",
    mitreName: "Data from Cloud Storage",
    severity: "high",
    source: "AWS CloudTrail",
    defaultParams: {
      username: "charlie@acme.com",
      source_ip: "172.16.0.22",
      region: "eu-west-1",
      severity: 0.85,
    },
    buildCloudTrail: (p) => ({
      Records: [
        {
          eventVersion: "1.08",
          userIdentity: { type: "IAMUser", userName: p.username },
          eventTime: nowIso(),
          eventSource: "s3.amazonaws.com",
          eventName: "PutBucketAcl",
          awsRegion: p.region,
          sourceIPAddress: p.source_ip,
          userAgent: "aws-sdk-python/1.26.0",
          requestParameters: {
            bucketName: "acme-prod-data-lake",
            AccessControlPolicy: {
              Grants: [
                {
                  Grantee: { Type: "Group", URI: "http://acs.amazonaws.com/groups/global/AllUsers" },
                  Permission: "READ",
                },
              ],
            },
          },
          responseElements: null,
        },
      ],
    }),
    buildAlert: (p) => ({
      case_class: "exfiltration",
      actor_user_name: p.username,
      source_ip: p.source_ip,
      cloud_region: p.region,
      severity_score: p.severity,
      source_class_id: 6003,
      entity_reputation_score: 0.2,
      baseline_deviation: 3.5,
      threat_intel_hit_count: 1,
      hour_of_day: new Date().getHours(),
      asset_criticality: 0.9,
      prior_disposition_ratio: 0.8,
    }),
  },

  {
    id: "mfa_bypass",
    title: "MFA factor bypass attempt",
    description:
      "A user successfully authenticated with primary credentials but attempted to skip or circumvent the MFA step, a common indicator of account takeover.",
    mitre: "T1556.006",
    mitreName: "Modify Authentication Process: MFA",
    severity: "critical",
    source: "Okta System Log",
    defaultParams: {
      username: "diana@acme.com",
      source_ip: "192.0.2.15",
      region: "us-west-2",
      severity: 0.95,
    },
    buildCloudTrail: (p) => ({
      Records: [
        {
          eventVersion: "1.08",
          userIdentity: { type: "IAMUser", userName: p.username },
          eventTime: nowIso(),
          eventSource: "okta.com",
          eventName: "user.mfa.factor.deactivate",
          awsRegion: p.region,
          sourceIPAddress: p.source_ip,
          userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
          requestParameters: { factorType: "token:software:totp" },
          responseElements: { outcome: { result: "SUCCESS" } },
          additionalEventData: { reason: "USER_BYPASS" },
        },
      ],
    }),
    buildAlert: (p) => ({
      case_class: "default",
      actor_user_name: p.username,
      source_ip: p.source_ip,
      cloud_region: p.region,
      severity_score: p.severity,
      source_class_id: 3002,
      entity_reputation_score: 0.3,
      baseline_deviation: 5.1,
      threat_intel_hit_count: 1,
      hour_of_day: new Date().getHours(),
      asset_criticality: 0.7,
      prior_disposition_ratio: 0.85,
    }),
  },
];

export function getScenario(id: string): Scenario | undefined {
  return SCENARIOS.find((s) => s.id === id);
}
