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

variable "anchor_retention_days" {
  type        = number
  description = "Default Object Lock COMPLIANCE retention (days) for the anchor bucket. Compliance mode cannot be shortened or removed by any principal, including the account root, until it expires."
  default     = 365

  validation {
    condition     = var.anchor_retention_days >= 1
    error_message = "anchor_retention_days must be at least 1."
  }
}

variable "warm_retention_days" {
  type        = number
  description = "Default Object Lock retention (days) for the warm bucket. 0 enables Object Lock but sets no default retention rule."
  default     = 0

  validation {
    condition     = var.warm_retention_days >= 0
    error_message = "warm_retention_days must be 0 or greater."
  }
}

variable "warm_retention_mode" {
  type        = string
  description = "Default Object Lock mode for the warm bucket when warm_retention_days > 0. GOVERNANCE by default so Iceberg compaction and expiry stay possible with explicit bypass; COMPLIANCE is irreversible."
  default     = "GOVERNANCE"

  validation {
    condition     = contains(["GOVERNANCE", "COMPLIANCE"], var.warm_retention_mode)
    error_message = "warm_retention_mode must be GOVERNANCE or COMPLIANCE."
  }
}

variable "kms_deletion_window_days" {
  type        = number
  description = "Waiting period (7-30 days) before a scheduled KMS key deletion takes effect."
  default     = 30

  validation {
    condition     = var.kms_deletion_window_days >= 7 && var.kms_deletion_window_days <= 30
    error_message = "kms_deletion_window_days must be between 7 and 30."
  }
}

variable "irsa_anchor_service_account" {
  type        = string
  description = "ServiceAccount allowed to write anchors (the attestation component). Separate from the warm-tier ServiceAccount."
  default     = "attest-anchor"
}

variable "irsa_verifier_service_account" {
  type        = string
  description = "ServiceAccount allowed to read anchors for offline verification. Read-only."
  default     = "attest-verifier"
}
