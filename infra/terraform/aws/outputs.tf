output "warm_bucket" {
  description = "S3 bucket for the Iceberg warm tier."
  value       = aws_s3_bucket.warm.id
}

output "warm_bucket_arn" {
  value = aws_s3_bucket.warm.arn
}

output "warm_iam_policy_arn" {
  description = "Attach this policy to the IRSA role used by storage-iceberg / collector."
  value       = aws_iam_policy.warm.arn
}
