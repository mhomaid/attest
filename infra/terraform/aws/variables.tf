variable "region" {
  type        = string
  description = "AWS region for the warm-tier bucket and optional IRSA role."
  default     = "us-east-1"
}

variable "name" {
  type        = string
  description = "Name prefix for BYOC resources."
  default     = "attest"
}

variable "force_destroy" {
  type        = bool
  description = "Allow terraform destroy to empty the warm bucket. Leave false outside throwaway accounts."
  default     = false
}

variable "oidc_provider_arn" {
  type        = string
  description = "EKS OIDC provider ARN. Empty skips IRSA — attach warm_iam_policy_arn yourself."
  default     = ""
}

variable "oidc_provider_url" {
  type        = string
  description = "EKS OIDC issuer URL without https:// (e.g. oidc.eks.us-east-1.amazonaws.com/id/EX). Required with oidc_provider_arn."
  default     = ""
}

variable "irsa_namespace" {
  type        = string
  description = "Kubernetes namespace the Helm release will use (IRSA subject)."
  default     = "attest"
}

variable "irsa_service_account" {
  type        = string
  description = "ServiceAccount name the Helm chart creates."
  default     = "attest"
}
