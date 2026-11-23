output "warm_bucket" {
  description = "S3 bucket for the Iceberg warm tier."
  value       = aws_s3_bucket.warm.id
}

output "warm_bucket_arn" {
  description = "ARN of the warm bucket."
  value       = aws_s3_bucket.warm.arn
}

output "warm_bucket_region" {
  description = "Region of the warm bucket."
  value       = var.region
}

output "warm_iam_policy_arn" {
  description = "Attach this to the IRSA role (or any role) used by storage-iceberg / collector."
  value       = aws_iam_policy.warm.arn
}

output "irsa_role_arn" {
  description = "Set this as serviceAccount.annotations.eks.amazonaws.com/role-arn on the Helm chart. Empty if OIDC vars were not set."
  value       = try(aws_iam_role.irsa[0].arn, "")
}

output "kms_key_arn" {
  description = "Customer-managed KMS key encrypting the warm and anchor buckets."
  value       = aws_kms_key.attest.arn
}

output "anchor_bucket" {
  description = "S3 bucket (Object Lock, COMPLIANCE mode) for attestation anchors."
  value       = aws_s3_bucket.anchors.id
}

output "anchor_bucket_arn" {
  description = "ARN of the anchor bucket."
  value       = aws_s3_bucket.anchors.arn
}

output "anchor_writer_policy_arn" {
  description = "Append-only anchor write policy, for roles you manage yourself."
  value       = aws_iam_policy.anchor_writer.arn
}

output "anchor_reader_policy_arn" {
  description = "Read-only anchor policy, for roles you manage yourself."
  value       = aws_iam_policy.anchor_reader.arn
}

output "anchor_irsa_role_arn" {
  description = "Role for the anchor-writing ServiceAccount. Empty if OIDC vars were not set."
  value       = try(aws_iam_role.anchor_irsa["anchor"].arn, "")
}

output "verifier_irsa_role_arn" {
  description = "Read-only role for the verifier ServiceAccount. Empty if OIDC vars were not set."
  value       = try(aws_iam_role.anchor_irsa["verifier"].arn, "")
}
