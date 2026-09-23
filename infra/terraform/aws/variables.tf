variable "region" {
  type        = string
  description = "AWS region for the warm-tier bucket and (optional) EKS data plane."
  default     = "us-east-1"
}

variable "name" {
  type        = string
  description = "Name prefix for BYOC resources."
  default     = "attest"
}

variable "cluster_name" {
  type        = string
  description = "Existing EKS cluster to install the Helm chart into. Empty skips the Helm release."
  default     = ""
}

variable "namespace" {
  type        = string
  description = "Kubernetes namespace for the Attest Helm release."
  default     = "attest"
}

variable "image_tag" {
  type        = string
  default     = "latest"
}

variable "signing_key" {
  type        = string
  description = "Hex-encoded Ed25519 seed for attestation. Leave empty only in throwaway envs."
  default     = ""
  sensitive   = true
}
