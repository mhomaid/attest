# Attest BYOC (GCP) — warm bucket

Creates a GCS bucket, a service account, and **HMAC keys** so the current
S3-only Iceberg writer can talk to GCS at `https://storage.googleapis.com`.

Does **not** create GKE, Kafka, RisingWave, or ClickHouse.

```sh
cd infra/terraform/gcp
cp terraform.tfvars.example terraform.tfvars
gcloud auth application-default login
terraform init
terraform plan
terraform apply
```

Point Helm at the outputs:

```sh
terraform output -raw s3_endpoint      # https://storage.googleapis.com
terraform output -raw warm_bucket
terraform output -raw s3_access_key
terraform output -raw s3_secret_key
```

Then [`../k8s-release`](../k8s-release) after `gcloud container clusters get-credentials`.
