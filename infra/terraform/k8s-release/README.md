# Helm onto any existing cluster (EKS / AKS / GKE)

Same chart as [`../eks-release`](../eks-release). This root uses your
**kubeconfig** instead of the AWS EKS API, so it works after:

```sh
aws eks update-kubeconfig --name attest-data --region us-east-1
# or
az aks get-credentials --name attest-data --resource-group attest-warm
# or
gcloud container clusters get-credentials attest-data --region us-central1
```

```sh
cd infra/terraform/k8s-release
terraform init
terraform apply \
  -var="warm_bucket=BUCKET" \
  -var="signing_key=$ATTEST_SIGNING_KEY"
```

GCP HMAC example:

```sh
terraform apply \
  -var="warm_bucket=$(terraform -chdir=../gcp output -raw warm_bucket)" \
  -var="object_store_endpoint=$(terraform -chdir=../gcp output -raw s3_endpoint)" \
  -var="object_store_access_key=$(terraform -chdir=../gcp output -raw s3_access_key)" \
  -var="object_store_secret_key=$(terraform -chdir=../gcp output -raw s3_secret_key)" \
  -var="signing_key=$ATTEST_SIGNING_KEY"
```
