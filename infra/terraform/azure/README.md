# Attest BYOC (Azure) — warm store

Creates a resource group (optional), a **ZRS** storage account with hierarchical
namespace (ADLS Gen2), a private container, and a user-assigned identity.

Does **not** create AKS, Kafka, RisingWave, or ClickHouse.

**Honest limit:** `attest-storage-iceberg` speaks the **S3 API** today. This
module still gives you the customer-owned store. Until a native Azure writer
exists, put MinIO (or another S3 gateway) in AKS and keep this account as the
durable target you will migrate to (`warehouse_abfss` output).

```sh
cd infra/terraform/azure
cp terraform.tfvars.example terraform.tfvars
az login
terraform init
terraform plan
terraform apply
```

```sh
terraform output warehouse_abfss
terraform output -raw identity_client_id
```

Then `az aks get-credentials` and [`../k8s-release`](../k8s-release).
