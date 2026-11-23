# Anchor bucket: write-once storage for attestation chain anchors (signed
# checkpoint digests). Object Lock runs in COMPLIANCE mode: for the whole
# retention period no principal, including the account root, can delete the
# object version or shorten its retention. That is what makes an anchor useful
# as tamper evidence, and it is irreversible. Pick anchor_retention_days
# deliberately; throwaway accounts should use a small value, and
# force_destroy does NOT bypass COMPLIANCE retention.
locals {
  anchor_bucket_name = "${var.name}-anchors-${data.aws_caller_identity.current.account_id}"
}

resource "aws_s3_bucket" "anchors" {
  bucket              = local.anchor_bucket_name
  force_destroy       = var.force_destroy
  object_lock_enabled = true

  tags = {
    Tier = "attestation-anchors"
  }
}

resource "aws_s3_bucket_versioning" "anchors" {
  bucket = aws_s3_bucket.anchors.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_object_lock_configuration" "anchors" {
  bucket = aws_s3_bucket.anchors.id

  rule {
    default_retention {
      mode = "COMPLIANCE"
      days = var.anchor_retention_days
    }
  }

  depends_on = [aws_s3_bucket_versioning.anchors]
}

resource "aws_s3_bucket_server_side_encryption_configuration" "anchors" {
  bucket = aws_s3_bucket.anchors.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm     = "aws:kms"
      kms_master_key_id = aws_kms_key.attest.arn
    }
    bucket_key_enabled = true
  }
}

resource "aws_s3_bucket_public_access_block" "anchors" {
  bucket                  = aws_s3_bucket.anchors.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_ownership_controls" "anchors" {
  bucket = aws_s3_bucket.anchors.id
  rule {
    object_ownership = "BucketOwnerEnforced"
  }
}

data "aws_iam_policy_document" "anchors_bucket" {
  statement {
    sid     = "DenyInsecureTransport"
    effect  = "Deny"
    actions = ["s3:*"]
    resources = [
      aws_s3_bucket.anchors.arn,
      "${aws_s3_bucket.anchors.arn}/*",
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

  # Nobody may lift retention early, even if an IAM policy would allow it.
  statement {
    sid       = "DenyBypassGovernanceRetention"
    effect    = "Deny"
    actions   = ["s3:BypassGovernanceRetention"]
    resources = ["${aws_s3_bucket.anchors.arn}/*"]
    principals {
      type        = "*"
      identifiers = ["*"]
    }
  }
}

resource "aws_s3_bucket_policy" "anchors" {
  bucket = aws_s3_bucket.anchors.id
  policy = data.aws_iam_policy_document.anchors_bucket.json
}

# Writer: append-only. No delete, no retention changes.
data "aws_iam_policy_document" "anchor_writer" {
  statement {
    sid       = "ListBucket"
    effect    = "Allow"
    actions   = ["s3:ListBucket", "s3:GetBucketLocation"]
    resources = [aws_s3_bucket.anchors.arn]
  }

  statement {
    sid       = "WriteAnchors"
    effect    = "Allow"
    actions   = ["s3:PutObject", "s3:GetObject", "s3:GetObjectRetention"]
    resources = ["${aws_s3_bucket.anchors.arn}/*"]
  }

  statement {
    sid       = "UseAttestKey"
    effect    = "Allow"
    actions   = ["kms:GenerateDataKey", "kms:Decrypt"]
    resources = [aws_kms_key.attest.arn]
  }
}

resource "aws_iam_policy" "anchor_writer" {
  name        = "${var.name}-anchor-writer"
  description = "Append-only write of Attest attestation anchors (no delete, no retention changes)."
  policy      = data.aws_iam_policy_document.anchor_writer.json
}

# Verifier: read-only, for offline verification of anchors.
data "aws_iam_policy_document" "anchor_reader" {
  statement {
    sid       = "ListBucket"
    effect    = "Allow"
    actions   = ["s3:ListBucket", "s3:GetBucketLocation"]
    resources = [aws_s3_bucket.anchors.arn]
  }

  statement {
    sid       = "ReadAnchors"
    effect    = "Allow"
    actions   = ["s3:GetObject", "s3:GetObjectRetention", "s3:GetObjectVersion"]
    resources = ["${aws_s3_bucket.anchors.arn}/*"]
  }

  statement {
    sid       = "DecryptWithAttestKey"
    effect    = "Allow"
    actions   = ["kms:Decrypt"]
    resources = [aws_kms_key.attest.arn]
  }
}

resource "aws_iam_policy" "anchor_reader" {
  name        = "${var.name}-anchor-reader"
  description = "Read-only access to Attest attestation anchors."
  policy      = data.aws_iam_policy_document.anchor_reader.json
}

locals {
  anchor_roles = local.irsa ? {
    anchor = {
      service_account = var.irsa_anchor_service_account
      policy_arn      = aws_iam_policy.anchor_writer.arn
    }
    verifier = {
      service_account = var.irsa_verifier_service_account
      policy_arn      = aws_iam_policy.anchor_reader.arn
    }
  } : {}
}

data "aws_iam_policy_document" "anchor_irsa_assume" {
  for_each = local.anchor_roles

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
      values   = ["system:serviceaccount:${var.irsa_namespace}:${each.value.service_account}"]
    }
    condition {
      test     = "StringEquals"
      variable = "${var.oidc_provider_url}:aud"
      values   = ["sts.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "anchor_irsa" {
  for_each           = local.anchor_roles
  name               = "${var.name}-${each.key}-irsa"
  assume_role_policy = data.aws_iam_policy_document.anchor_irsa_assume[each.key].json
}

resource "aws_iam_role_policy_attachment" "anchor_irsa" {
  for_each   = local.anchor_roles
  role       = aws_iam_role.anchor_irsa[each.key].name
  policy_arn = each.value.policy_arn
}
