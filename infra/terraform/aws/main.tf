data "aws_caller_identity" "current" {}

resource "aws_s3_bucket" "warm" {
  bucket = "${var.name}-warm-${data.aws_caller_identity.current.account_id}"

  tags = {
    Product = "attest"
    Tier    = "iceberg-warm"
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
  }
}

resource "aws_s3_bucket_public_access_block" "warm" {
  bucket                  = aws_s3_bucket.warm.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

data "aws_iam_policy_document" "warm" {
  statement {
    sid     = "AttestIcebergReadWrite"
    effect  = "Allow"
    actions = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:ListBucket"]
    resources = [
      aws_s3_bucket.warm.arn,
      "${aws_s3_bucket.warm.arn}/*",
    ]
  }
}

resource "aws_iam_policy" "warm" {
  name   = "${var.name}-iceberg-warm"
  policy = data.aws_iam_policy_document.warm.json
}

# Optional: install the reference Helm chart onto an existing EKS cluster.
data "aws_eks_cluster" "this" {
  count = var.cluster_name == "" ? 0 : 1
  name  = var.cluster_name
}

data "aws_eks_cluster_auth" "this" {
  count = var.cluster_name == "" ? 0 : 1
  name  = var.cluster_name
}

provider "kubernetes" {
  alias                  = "eks"
  host                   = try(data.aws_eks_cluster.this[0].endpoint, null)
  cluster_ca_certificate = try(base64decode(data.aws_eks_cluster.this[0].certificate_authority[0].data), null)
  token                  = try(data.aws_eks_cluster_auth.this[0].token, null)
}

provider "helm" {
  alias = "eks"
  kubernetes {
    host                   = try(data.aws_eks_cluster.this[0].endpoint, null)
    cluster_ca_certificate = try(base64decode(data.aws_eks_cluster.this[0].certificate_authority[0].data), null)
    token                  = try(data.aws_eks_cluster_auth.this[0].token, null)
  }
}

resource "helm_release" "attest" {
  count      = var.cluster_name == "" ? 0 : 1
  provider   = helm.eks
  name       = "attest"
  namespace  = var.namespace
  chart      = "${path.module}/../../helm/attest"
  create_namespace = true

  set {
    name  = "image.tag"
    value = var.image_tag
  }
  set {
    name  = "objectStore.bucket"
    value = aws_s3_bucket.warm.id
  }
  set {
    name  = "objectStore.region"
    value = var.region
  }
  set {
    name  = "env.ICEBERG_WAREHOUSE"
    value = "s3://${aws_s3_bucket.warm.id}"
  }
  set_sensitive {
    name  = "signingKey"
    value = var.signing_key
  }
}
