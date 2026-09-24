variable "project" {
  type        = string
  description = "GCP project id."
}

variable "location" {
  type        = string
  description = "Bucket location (region or multi-region, e.g. us-central1 or US)."
  default     = "us-central1"
}

variable "name" {
  type        = string
  description = "Name prefix. Bucket becomes {name}-warm-{project_number}."
  default     = "attest"
}

variable "force_destroy" {
  type    = bool
  default = false
}

variable "k8s_namespace" {
  type        = string
  description = "GKE namespace for Workload Identity. Empty skips WI binding."
  default     = ""
}

variable "k8s_service_account" {
  type    = string
  default = "attest"
}
