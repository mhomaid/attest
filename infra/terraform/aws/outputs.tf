output "warm_bucket" {
  description = "S3 bucket for the Iceberg warm tier."
  value       = aws_s3_bucket.warm.id
}

output "warm_bucket_arn" {
  value = aws_s3_bucket.warm.arn
}

output "warm_bucket_region" {
  value = var.region
}

output "warm_iam_policy_arn" {
  description = "Attach this to the IRSA role (or any role) used by storage-iceberg / collector."
  value       = aws_iam_policy.warm.arn
}

output "irsa_role_arn" {
  description = "Set this as serviceAccount.annotations.eks.amazonaws.com/role-arn on the Helm chart. Empty if OIDC vars were not set."
  value       = try(aws_iam_role.irsa[0].arn, "")
}
