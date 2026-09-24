variable "location" {
  type        = string
  description = "Azure region, e.g. eastus."
  default     = "eastus"
}

variable "name" {
  type        = string
  description = "Name prefix. Storage account is {name}warm{8-hex} (must be globally unique)."
  default     = "attest"
}

variable "resource_group_name" {
  type        = string
  description = "Existing resource group. Empty creates {name}-warm."
  default     = ""
}

variable "replication_type" {
  type        = string
  description = "Storage replication. GRS works in more regions than ZRS."
  default     = "GRS"
}

variable "k8s_oidc_issuer_url" {
  type        = string
  description = "AKS OIDC issuer URL. Empty skips federated identity."
  default     = ""
}

variable "k8s_namespace" {
  type    = string
  default = "attest"
}

variable "k8s_service_account" {
  type    = string
  default = "attest"
}
