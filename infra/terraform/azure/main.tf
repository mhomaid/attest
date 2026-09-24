data "azurerm_client_config" "current" {}

locals {
  rg_name = var.resource_group_name != "" ? var.resource_group_name : "${var.name}-warm"
  # Storage account: 3–24 lowercase alphanumeric, globally unique.
  sa_name  = substr(replace("${var.name}warm${substr(md5(data.azurerm_client_config.current.subscription_id), 0, 8)}", "-", ""), 0, 24)
  federate = var.k8s_oidc_issuer_url != ""
}

resource "azurerm_resource_group" "this" {
  count    = var.resource_group_name == "" ? 1 : 0
  name     = local.rg_name
  location = var.location
  tags = {
    product = "attest"
    tier    = "iceberg-warm"
  }
}

data "azurerm_resource_group" "this" {
  count = var.resource_group_name == "" ? 0 : 1
  name  = var.resource_group_name
}

locals {
  rg = var.resource_group_name == "" ? azurerm_resource_group.this[0] : data.azurerm_resource_group.this[0]
}

resource "azurerm_storage_account" "warm" {
  name                            = local.sa_name
  resource_group_name             = local.rg.name
  location                        = local.rg.location
  account_tier                    = "Standard"
  account_replication_type        = var.replication_type
  min_tls_version                 = "TLS1_2"
  https_traffic_only_enabled      = true
  allow_nested_items_to_be_public = false
  shared_access_key_enabled       = true
  is_hns_enabled                  = true
  tags = {
    product = "attest"
    tier    = "iceberg-warm"
  }

  blob_properties {
    versioning_enabled = true
  }
}

resource "azurerm_storage_container" "warm" {
  name                  = "attest-warm"
  storage_account_id    = azurerm_storage_account.warm.id
  container_access_type = "private"
}

resource "azurerm_user_assigned_identity" "warm" {
  name                = "${var.name}-warm"
  resource_group_name = local.rg.name
  location            = local.rg.location
}

resource "azurerm_role_assignment" "blob" {
  scope                = azurerm_storage_account.warm.id
  role_definition_name = "Storage Blob Data Contributor"
  principal_id         = azurerm_user_assigned_identity.warm.principal_id
}

resource "azurerm_federated_identity_credential" "aks" {
  count               = local.federate ? 1 : 0
  name                = "${var.name}-warm-aks"
  resource_group_name = local.rg.name
  parent_id           = azurerm_user_assigned_identity.warm.id
  audience            = ["api://AzureADTokenExchange"]
  issuer              = var.k8s_oidc_issuer_url
  subject             = "system:serviceaccount:${var.k8s_namespace}:${var.k8s_service_account}"
}
