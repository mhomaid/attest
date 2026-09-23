# Attest BYOC (AWS)

Provisions the customer-owned Iceberg warm bucket and, optionally, installs the
`infra/helm/attest` chart onto an existing EKS cluster.

```sh
terraform init
terraform plan -var="region=us-east-1"
# After an EKS cluster exists:
terraform apply -var="cluster_name=attest-data" -var="signing_key=$ATTEST_SIGNING_KEY"
```

This module does **not** create EKS, Redpanda, ClickHouse, or RisingWave. Those
stay the customer's (or the in-cluster Helm charts they already run). The
bucket + IRSA policy is the piece Attest must own in every BYOC deal.
