const rules = [
  {
    id: "aws_console_login_from_anomalous_geolocation",
    title: "Console login from anomalous region",
    mitre: "T1078.004",
    severity: "medium",
    source: "CloudTrail",
  },
  {
    id: "aws_cloudtrail_logging_disabled",
    title: "CloudTrail logging disabled",
    mitre: "T1562.001",
    severity: "critical",
    source: "CloudTrail",
  },
  {
    id: "aws_root_account_use",
    title: "AWS root account used",
    mitre: "T1078",
    severity: "critical",
    source: "CloudTrail",
  },
  {
    id: "aws_iam_excessive_privilege",
    title: "Excessive IAM privilege granted",
    mitre: "T1098",
    severity: "high",
    source: "CloudTrail",
  },
  {
    id: "aws_new_iam_user_then_keys",
    title: "New IAM user + access keys",
    mitre: "T1136 + T1098",
    severity: "high",
    source: "CloudTrail",
  },
  {
    id: "aws_s3_bucket_made_public",
    title: "S3 bucket made public",
    mitre: "T1530",
    severity: "high",
    source: "CloudTrail",
  },
  {
    id: "okta_brute_force",
    title: "Okta brute-force authentication",
    mitre: "T1110",
    severity: "high",
    source: "Okta",
  },
  {
    id: "okta_mfa_bypass",
    title: "Okta MFA bypass attempt",
    mitre: "T1556.006",
    severity: "high",
    source: "Okta",
  },
  {
    id: "m365_mass_external_sharing",
    title: "M365 mass external sharing",
    mitre: "T1567.002",
    severity: "high",
    source: "M365",
  },
  {
    id: "m365_inbox_auto_forward",
    title: "M365 inbox auto-forward",
    mitre: "T1564.008",
    severity: "high",
    source: "M365",
  },
] as const;

const sample = `detection: aws_console_login_from_anomalous_geolocation
where:
  - event.auth_status = "Success"
condition:
  - event.cloud_region NOT IN baseline(identity.user, 90d)
severity: medium
mitre: [T1078.004]
runtime: stream | batch`;

export function DetectionsCatalogSection() {
  return (
    <section id="detections" className="border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          HELIQL v0
        </p>
        <h2 className="mt-2 text-3xl font-semibold tracking-tight sm:text-4xl">
          Ten rules that compile to the stream
        </h2>
        <p className="mt-4 max-w-3xl text-muted-foreground">
          pest grammar → typed AST → RisingWave{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs">
            CREATE MATERIALIZED VIEW
          </code>
          . Baseline references become correlated subqueries against{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs">
            entity_baselines
          </code>
          . Sigma YAML imports into the same AST. Detection-runtime polls
          fired rows every 2s onto the{" "}
          <code className="rounded bg-secondary px-1 py-0.5 font-mono text-xs">
            alerts
          </code>{" "}
          topic.
        </p>

        <pre className="mt-8 overflow-x-auto rounded-xl border border-border/70 bg-card/60 p-4 font-mono text-[11px] leading-relaxed text-muted-foreground sm:text-xs">
          {sample}
        </pre>

        <div className="mt-8 overflow-x-auto rounded-xl border border-border/70">
          <table className="w-full min-w-[40rem] text-left text-sm">
            <thead className="border-b border-border/70 bg-secondary/40 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
              <tr>
                <th className="px-3 py-2 font-medium">Rule</th>
                <th className="px-3 py-2 font-medium">Source</th>
                <th className="px-3 py-2 font-medium">ATT&CK</th>
                <th className="px-3 py-2 font-medium">Severity</th>
              </tr>
            </thead>
            <tbody>
              {rules.map((rule) => (
                <tr
                  key={rule.id}
                  className="border-b border-border/50 last:border-0"
                >
                  <td className="px-3 py-2.5">
                    <p className="font-medium">{rule.title}</p>
                    <p className="font-mono text-[10px] text-muted-foreground">
                      {rule.id}
                    </p>
                  </td>
                  <td className="px-3 py-2.5 text-muted-foreground">
                    {rule.source}
                  </td>
                  <td className="px-3 py-2.5 font-mono text-xs">{rule.mitre}</td>
                  <td className="px-3 py-2.5 font-mono text-xs">{rule.severity}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}
