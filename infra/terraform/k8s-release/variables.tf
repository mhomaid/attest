variable "kubeconfig" {
  type        = string
  description = "Path to kubeconfig. Default is the local CLI context."
  default     = "~/.kube/config"
}

variable "kube_context" {
  type        = string
  description = "Kube context name. Empty uses the current context."
  default     = ""
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
  description = "Bucket / container name the writer should use."
}

variable "object_store_endpoint" {
  type        = string
  description = "S3-compatible endpoint. AWS: leave empty (SDK default). GCP: https://storage.googleapis.com. Azure: your in-cluster MinIO until native Blob exists."
  default     = ""
}

variable "object_store_region" {
  type    = string
  default = "us-east-1"
}

variable "object_store_access_key" {
  type      = string
  default   = ""
  sensitive = true
}

variable "object_store_secret_key" {
  type      = string
  default   = ""
  sensitive = true
}

variable "use_irsa" {
  type        = bool
  description = "AWS IRSA / skip static keys."
  default     = false
}

variable "signing_key" {
  type        = string
  description = "Hex-encoded Ed25519 seed."
  sensitive   = true
}

variable "service_account_annotations" {
  type        = map(string)
  description = "IRSA / Workload Identity annotations on the chart ServiceAccount."
  default     = {}
}
