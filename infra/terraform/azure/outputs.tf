output "resource_group" {
  value = local.rg.name
}

output "storage_account" {
  value = azurerm_storage_account.warm.name
}

output "container" {
  value = azurerm_storage_container.warm.name
}

output "blob_endpoint" {
  value = azurerm_storage_account.warm.primary_blob_endpoint
}

output "warehouse_abfss" {
  description = "ADLS path for a future native Azure writer. The current Iceberg writer is S3-only."
  value       = "abfss://${azurerm_storage_container.warm.name}@${azurerm_storage_account.warm.name}.dfs.core.windows.net/"
}

output "identity_client_id" {
  description = "User-assigned identity. Helm annotation azure.workload.identity/client-id when federated."
  value       = azurerm_user_assigned_identity.warm.client_id
}

output "identity_id" {
  value = azurerm_user_assigned_identity.warm.id
}

# Account keys exist so you can stand up an S3-compatible gateway (MinIO) in AKS
# until the writer speaks Azure. Do not put these in git.
output "storage_access_key" {
  value     = azurerm_storage_account.warm.primary_access_key
  sensitive = true
}
