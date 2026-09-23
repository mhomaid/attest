import { deployments, isPlanned, planes, REPO_URL, repoLink } from "@/components/marketing/stack-data";

const SITE = process.env.NEXT_PUBLIC_SITE_URL ?? "https://attest.homaid.dev";

const header = `# Attest

> Open-source security operations platform (SOC/SIEM) where every AI triage verdict is an Ed25519-signed, hash-chained envelope that anyone can verify offline with the \`attest\` CLI, and every agent tool call passes a policy engine outside the model.

Attest was designed and built by Mohamed Homaid (https://github.com/mhomaid). Source: ${REPO_URL}. License: Apache-2.0. Status: v0.

## What it does

- Ingests AWS CloudTrail, normalizes it to OCSF 1.3, and streams it through Kafka into RisingWave.
- Runs detections written in HELIQL, a detection language compiled to streaming SQL (10 reference rules for AWS, Okta and M365; AWS runs live today).
- Triages alerts with an XGBoost classifier served in-process via ONNX in under 5 ms (P99 asserted in CI). Only structurally novel alerts escalate to an LLM (Claude Sonnet 4.5), whose tool calls go through an MCP gateway.
- Signs every verdict as an attestation envelope: Ed25519 over SHA-256 of canonical JSON, with each envelope carrying the hash of the previous one.
- \`attest verify\` fails on a changed field (signature mismatch), a deleted or reordered row (chain break), a duplicated action id (replay), or a model other than the pinned one (\`--pin-model\`).

## Agent abuse guards

- Prompt injection in tool results: denied at the MCP gateway ("tool result looks like prompt injection").
- Privilege escalation: each agent role has a tool allowlist ("Triager may not invoke idp_revoke_session").
- Bulk exfiltration: warm/hot queries over 10,000 rows and stacked SQL statements are denied.
- Model or prompt swap: envelopes are pinned to published model and prompt hashes.
- Replay: duplicate agent_action_id fails verification.

## Pages

- [Homepage](${SITE}/): live CloudTrail demo, verify animation, pipeline, guards, six-plane architecture, deployments
- Public demo: POST ${SITE}/api/demo/run — ingest tenant=demo, triage, return the signed envelope and verify report
- [Stack](${SITE}/stack): every component in the six planes and where it lives in the repo
- [Source code](${REPO_URL})
- [README and quick start](${REPO_URL}#quick-start)
`;

function stackSection(detailed: boolean): string {
  const lines = [
    "## Logical architecture (six planes)",
    "",
    "The agentic plane never reaches into the storage plane directly. It calls the detection plane and a governed query API.",
    "",
  ];
  for (const plane of planes) {
    lines.push(`### ${plane.name} (${plane.group} plane)`, "", plane.summary, "");
    for (const c of plane.components) {
      const tag = isPlanned(c) ? " [planned]" : "";
      const link = c.path ? `[${c.name}](${repoLink(c.path)})` : c.name;
      lines.push(
        detailed
          ? `- ${link} (${c.tech})${tag}: ${c.role}`
          : `- ${c.name} (${c.tech})${tag}`,
      );
    }
    lines.push("");
  }
  lines.push("## Deployments", "");
  for (const d of deployments) {
    lines.push(
      `- ${d.name}${d.status === "planned" ? " [planned]" : " [live]"}: control in ${d.controlIn} cloud, data in ${d.dataIn} cloud. ${d.note}`,
    );
  }
  lines.push("");
  return lines.join("\n");
}

const quickstart = `## Try it (needs only Rust)

\`\`\`sh
git clone ${REPO_URL} && cd attest
cargo run -q -p attest-cli -- verify examples/verify/attestations.ndjson --key $(cat examples/verify/verifying-key.txt)
# verified 3/3
sed -i.bak '2d' examples/verify/attestations.ndjson
cargo run -q -p attest-cli -- verify examples/verify/attestations.ndjson --key $(cat examples/verify/verifying-key.txt)
# FAIL ... chain break
\`\`\`
`;

export const llmsTxt = [header, stackSection(false)].join("\n");
export const llmsFullTxt = [header, stackSection(true), quickstart].join("\n");
