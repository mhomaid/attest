terraform {
  required_version = ">= 1.6.0"

  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.17"
    }
  }
}

# Works for EKS, AKS, and GKE after you have a kubeconfig:
#   aws eks update-kubeconfig --name …
#   az aks get-credentials --name … --resource-group …
#   gcloud container clusters get-credentials …
provider "helm" {
  kubernetes {
    config_path    = var.kubeconfig
    config_context = var.kube_context == "" ? null : var.kube_context
  }
}
