variable "region" {
  type    = string
  default = "us-east-1"
}

variable "cluster_name" {
  type        = string
  description = "Existing EKS cluster. This module does not create a cluster."
}

variable "namespace" {
  type    = string
  default = "attest"
}

variable "image_tag" {
  type    = string
  default = "0.1.0"
}

variable "warm_bucket" {
  type        = string
  description = "Bucket id from the aws module (terraform -chdir=../aws output -raw warm_bucket)."
}

variable "irsa_role_arn" {
  type        = string
  description = "IRSA role ARN from the aws module. Empty uses static keys in a Secret (dev only)."
  default     = ""
}

variable "signing_key" {
  type        = string
  description = "Hex-encoded Ed25519 seed. Required for any deploy whose envelopes you will verify later."
  sensitive   = true
}
