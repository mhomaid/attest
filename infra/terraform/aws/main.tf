data "aws_caller_identity" "current" {}

locals {
  bucket_name = "${var.name}-warm-${data.aws_caller_identity.current.account_id}"
  irsa        = var.oidc_provider_arn != "" && var.oidc_provider_url != ""
}

resource "aws_s3_bucket" "warm" {
  bucket        = local.bucket_name
  force_destroy = var.force_destroy

  tags = {
    Tier = "iceberg-warm"
  }
}

resource "aws_s3_bucket_versioning" "warm" {
  bucket = aws_s3_bucket.warm.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "warm" {
  bucket = aws_s3_bucket.warm.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
    bucket_key_enabled = true
  }
}

resource "aws_s3_bucket_public_access_block" "warm" {
  bucket                  = aws_s3_bucket.warm.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_ownership_controls" "warm" {
  bucket = aws_s3_bucket.warm.id
  rule {
    object_ownership = "BucketOwnerEnforced"
  }
}

resource "aws_s3_bucket_lifecycle_configuration" "warm" {
  bucket = aws_s3_bucket.warm.id

  rule {
    id     = "abort-incomplete-multipart"
    status = "Enabled"
    filter {}
    abort_incomplete_multipart_upload {
      days_after_initiation = 7
    }
  }
}

data "aws_iam_policy_document" "tls_only" {
  statement {
    sid     = "DenyInsecureTransport"
    effect  = "Deny"
    actions = ["s3:*"]
    resources = [
      aws_s3_bucket.warm.arn,
      "${aws_s3_bucket.warm.arn}/*",
    ]
    principals {
      type        = "*"
      identifiers = ["*"]
    }
    condition {
      test     = "Bool"
      variable = "aws:SecureTransport"
      values   = ["false"]
    }
  }
}

resource "aws_s3_bucket_policy" "warm" {
  bucket = aws_s3_bucket.warm.id
  policy = data.aws_iam_policy_document.tls_only.json
}

data "aws_iam_policy_document" "warm" {
  statement {
    sid       = "ListBucket"
    effect    = "Allow"
    actions   = ["s3:ListBucket", "s3:GetBucketLocation"]
    resources = [aws_s3_bucket.warm.arn]
  }

  statement {
    sid       = "ObjectReadWrite"
    effect    = "Allow"
    actions   = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:AbortMultipartUpload"]
    resources = ["${aws_s3_bucket.warm.arn}/*"]
  }
}

resource "aws_iam_policy" "warm" {
  name        = "${var.name}-iceberg-warm"
  description = "Iceberg / collector read-write on the Attest warm bucket."
  policy      = data.aws_iam_policy_document.warm.json
}

data "aws_iam_policy_document" "irsa_assume" {
  count = local.irsa ? 1 : 0

  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRoleWithWebIdentity"]
    principals {
      type        = "Federated"
      identifiers = [var.oidc_provider_arn]
    }
    condition {
      test     = "StringEquals"
      variable = "${var.oidc_provider_url}:sub"
      values   = ["system:serviceaccount:${var.irsa_namespace}:${var.irsa_service_account}"]
    }
    condition {
      test     = "StringEquals"
      variable = "${var.oidc_provider_url}:aud"
      values   = ["sts.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "irsa" {
  count              = local.irsa ? 1 : 0
  name               = "${var.name}-warm-irsa"
  assume_role_policy = data.aws_iam_policy_document.irsa_assume[0].json
}

resource "aws_iam_role_policy_attachment" "irsa" {
  count      = local.irsa ? 1 : 0
  role       = aws_iam_role.irsa[0].name
  policy_arn = aws_iam_policy.warm.arn
}
