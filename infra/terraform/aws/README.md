# Attest BYOC (AWS) — warm bucket

Creates the customer-owned Iceberg warm bucket and an IAM policy. Optionally
creates an IRSA role for the Helm ServiceAccount. **Does not create EKS**,
Redpanda, ClickHouse, or RisingWave.

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
