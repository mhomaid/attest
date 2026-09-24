# Attest Helm release on existing EKS

Applies `infra/helm/attest` to an existing **EKS** cluster (AWS API, no
kubeconfig). For AKS / GKE use [`../k8s-release`](../k8s-release).

Run [`../aws`](../aws) first so the warm bucket (and optional IRSA role) exist.

Helm provider is pinned to `~> 2.17` (3.x dropped the nested `kubernetes {}` block).

```sh
cd infra/terraform/aws
terraform apply

cd ../eks-release
terraform init
terraform apply \
  -var="cluster_name=attest-data" \
  -var="warm_bucket=$(terraform -chdir=../aws output -raw warm_bucket)" \
  -var="irsa_role_arn=$(terraform -chdir=../aws output -raw irsa_role_arn)" \
  -var="signing_key=$ATTEST_SIGNING_KEY"
```
