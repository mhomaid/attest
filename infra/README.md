# Infra

Attest’s deployable pieces. Nothing here is a hosted multi-tenant product.
The **writer talks S3 today** (`AmazonS3Builder`). AWS is native. GCP is S3
via HMAC. Azure provisions a customer-owned ADLS account; the binary does
not speak Blob yet.

```
Local laptop          Compose (pinned images)     make dev-up-all
Hosted lab            Railway workbench only      make prod down|up
Customer cloud        Terraform store + Helm      AWS / GCP / Azure
```

---

## What lives here

| Path | What it is |
|---|---|
| [`../docker-compose.yml`](../docker-compose.yml) | Local stack. Infra images are **pinned**. |
| [`db/`](db/) | Alembic + SQL (Better Auth). |
| [`docker/`](docker/) | Multi-stage images for the Rust / Python services. |
| [`helm/attest`](helm/attest) | App chart only. Does **not** install Kafka, RisingWave, ClickHouse, or a store. |
| [`terraform/aws`](terraform/aws) | S3 warm bucket + IAM + optional IRSA. |
| [`terraform/gcp`](terraform/gcp) | GCS bucket + HMAC (S3 API) + optional Workload Identity. |
| [`terraform/azure`](terraform/azure) | ADLS Gen2 account + identity + optional AKS federation. |
| [`terraform/k8s-release`](terraform/k8s-release) | Helm onto **any** existing cluster (kubeconfig). |
| [`terraform/eks-release`](terraform/eks-release) | Same chart, talks to EKS by cluster name (no kubeconfig). |
| [`railway/`](railway/) | Hosted lab. Keep workbench; park the rest. |
| [`clickhouse/`](clickhouse/), [`risingwave/`](risingwave/), [`arroyo/`](arroyo/) | SQL Compose mounts. |

`make helm-template` and `make tf-validate` are the checks. Run both before you
push infra changes.

---

## Use it

### Laptop

```sh
make dev-up-all
# Auth schema: see the root README quick start
```

Images are pinned in Compose. After you pull a new pin, a RisingWave volume
from `:latest` can fail with `cluster_id` — delete the `risingwave` MinIO
bucket / volume and start again.

### Hosted lab (Railway)

```sh
make prod status
make prod down    # everything except workbench (attest.homaid.dev)
make prod up      # only when you need the data plane
```

Do not run `railway down` without `--service`. Do not use an old
`make railway-stop` that listed workbench.

### BYOC — pick one cloud, then Helm

Each store module is a **separate Terraform root**. `plan` does not need a
cluster. None of them create EKS / AKS / GKE.

**1. AWS (native S3)**

```sh
cd infra/terraform/aws
cp terraform.tfvars.example terraform.tfvars   # edit
terraform init && terraform plan && terraform apply
terraform output -raw warm_bucket
terraform output -raw irsa_role_arn            # empty unless you set OIDC vars
```

**2. GCP (GCS via S3 HMAC)**

```sh
cd infra/terraform/gcp
cp terraform.tfvars.example terraform.tfvars   # set project
gcloud auth application-default login
terraform init && terraform plan && terraform apply
terraform output -raw s3_endpoint              # https://storage.googleapis.com
terraform output -raw warm_bucket
terraform output -raw s3_access_key
terraform output -raw s3_secret_key
```

**3. Azure (ADLS provisioned; writer still S3)**

```sh
cd infra/terraform/azure
cp terraform.tfvars.example terraform.tfvars
az login
terraform init && terraform plan && terraform apply
terraform output warehouse_abfss
```

Stand up MinIO (or another S3 endpoint) in AKS until the Iceberg writer
gains Azure. Keep the storage account — that is the customer-owned store.

**4. Install the app on an existing cluster**

```sh
# context
aws eks update-kubeconfig --name YOUR_CLUSTER
# or: az aks get-credentials --name … --resource-group …
# or: gcloud container clusters get-credentials … --region …

cd infra/terraform/k8s-release
terraform init
terraform apply \
  -var="warm_bucket=BUCKET" \
  -var="signing_key=$ATTEST_SIGNING_KEY"
```

On EKS you can use [`terraform/eks-release`](terraform/eks-release) instead
(it reads the cluster from AWS). Set `ATTEST_SIGNING_KEY` or envelopes will
not verify after a restart.

Helm values that matter:

| Value | AWS | GCP | Azure today |
|---|---|---|---|
| `objectStore.bucket` | S3 bucket | GCS name | MinIO bucket |
| `objectStore.endpoint` | leave empty | `https://storage.googleapis.com` | in-cluster MinIO |
| `objectStore.useIrsa` | `true` with IRSA | `false` (HMAC) or WI later | `false` |
| `signingKey` | required for audit | same | same |

---

## Update it

Change the smallest thing that matches the job.

| You want to… | Edit | Then run |
|---|---|---|
| Pin / bump a **local** infra image | `docker-compose.yml` (and the same tag in the `Makefile` Railway recipes) | `make dev-up-all` on a throwaway compose project first |
| Change Helm probes, SA, ingress, resources | `helm/attest/values.yaml` + `templates/` | `make helm-template` |
| Change the **AWS** store | `terraform/aws/*.tf` | `make tf-validate` then `terraform plan` in that root |
| Change the **GCP** store | `terraform/gcp/*.tf` | same |
| Change the **Azure** store | `terraform/azure/*.tf` | same |
| Change how the chart is installed | `terraform/k8s-release` or `eks-release` | `terraform fmt` + `terraform validate` in that root |
| Add a Compose env / port | `docker-compose.yml` + the matching `env.example` | don’t commit secrets |
| Railway park / keep list | `scripts/prod-railway.sh` `KEEP` / `APPS` / `INFRA` | never add `workbench` to a down list |

Rules:

1. **One cloud root per change.** Do not merge AWS + GCP into one state file.
2. **Pin providers** with an upper bound (`< 7.0`, Helm `~> 2.17`). Helm 3.x
   dropped the nested `kubernetes {}` block.
3. **No `:latest`** on infra images. App images are built from
   `infra/docker/*`.
4. **`terraform.tfvars` is local.** Commit only `*.tfvars.example`.
5. After a Terraform apply, put outputs into Helm — do not hard-code account
   ids in the chart.
6. If you add a third store (e.g. native Azure writer), add a row to the
   table above and a Status line in the root README.

```sh
make helm-template
make tf-validate
```

`tf-validate` formats and validates **aws, gcp, and azure** (no credentials).
It does not apply anything.

### Limits this walkthrough still has

- The Iceberg writer is S3-API only. GCP HMAC is a real path. Azure ADLS is
  provisioned; pods still need MinIO (or another S3 endpoint) until a native
  writer exists. GKE Workload Identity does **not** replace HMAC today.
- Rust service images run as root. Helm does not set `runAsNonRoot`.
- Helm does not install Kafka, RisingWave, ClickHouse, or MinIO.
- `llama.cpp:server` on the Compose `llm` profile is still a moving tag.
