output "warm_bucket" {
  description = "GCS bucket name. Also the S3 bucket name when using HMAC."
  value       = google_storage_bucket.warm.name
}

output "s3_endpoint" {
  description = "Set Helm objectStore.endpoint to this. Writer is S3-API only."
  value       = "https://storage.googleapis.com"
}

output "s3_access_key" {
  description = "HMAC access id. Helm objectStore.accessKey."
  value       = google_storage_hmac_key.warm.access_id
  sensitive   = true
}

output "s3_secret_key" {
  description = "HMAC secret. Helm objectStore.secretKey."
  value       = google_storage_hmac_key.warm.secret
  sensitive   = true
}

output "service_account_email" {
  value = google_service_account.warm.email
}

output "helm_annotation_workload_identity" {
  description = "iam.gke.io/gcp-service-account on the chart ServiceAccount. Empty unless k8s_namespace was set."
  value       = local.wi ? google_service_account.warm.email : ""
}
