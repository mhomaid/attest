# Attest BYOC (AWS) — security account

Creates, in the account you point it at (example account ID `123456789012`):

- a customer-managed **KMS key** (rotation on, alias `alias/<name>`) used by both buckets;
- the Iceberg **warm bucket** (`<name>-warm-<account>`, e.g. `attest-warm-123456789012`),
  KMS-encrypted, versioned, Object Lock enabled;
- the **anchor bucket** (`<name>-anchors-<account>`, e.g. `attest-anchors-123456789012`)
  for attestation anchors, with Object Lock in **COMPLIANCE** mode;
- IAM policies, and optionally **IRSA roles** (warm, anchor writer, verifier) for the Helm ServiceAccounts.

**Does not create EKS**, Redpanda, ClickHouse, or RisingWave.

## Object Lock

| Bucket | Default retention | Mode |
| --- | --- | --- |
| anchors | `anchor_retention_days` (365) | `COMPLIANCE` |
| warm | `warm_retention_days` (0 = lock enabled, no default rule) | `warm_retention_mode` (`GOVERNANCE`) |

**COMPLIANCE mode is irreversible.** Until retention expires, no principal, including
the account root, can delete a locked object version, overwrite its retention, or
shorten it; `force_destroy` does not bypass it, and the bucket cannot be emptied or
destroyed while locked versions remain. The anchor bucket policy also denies
`s3:BypassGovernanceRetention`. Use a small `anchor_retention_days` in test accounts.
Object Lock must be set when a bucket is created; adding it to an existing warm bucket
from earlier versions of this module may require re-creating it.

## IRSA roles (when `oidc_provider_arn` and `oidc_provider_url` are set)

| Role | ServiceAccount var | Access |
| --- | --- | --- |
| `<name>-warm-irsa` | `irsa_service_account` | warm bucket read/write, KMS use |
| `<name>-anchor-irsa` | `irsa_anchor_service_account` | anchor bucket put/get only (no delete, no retention changes) |
| `<name>-verifier-irsa` | `irsa_verifier_service_account` | anchor bucket read-only |

Lint: `tflint --config ../.tflint.hcl` (run by `make tf-validate` when tflint is installed).

```sh
cd infra/terraform/aws
cp terraform.tfvars.example terraform.tfvars   # edit
terraform init
terraform plan
terraform apply
```

`terraform plan` works with no cluster. That is intentional.

Siblings: [`../gcp`](../gcp) (GCS + HMAC), [`../azure`](../azure) (ADLS).
How to use and update all three: [`../../README.md`](../../README.md).

To install the chart onto an **existing** EKS cluster, use
[`../eks-release`](../eks-release) or kubeconfig [`../k8s-release`](../k8s-release).

```sh
terraform output -raw warm_bucket
terraform output -raw irsa_role_arn
```
