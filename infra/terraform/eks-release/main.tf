resource "helm_release" "attest" {
  name             = "attest"
  namespace        = var.namespace
  chart            = "${path.module}/../../helm/attest"
  create_namespace = true

  set {
    name  = "image.tag"
    value = var.image_tag
  }
  set {
    name  = "objectStore.bucket"
    value = var.warm_bucket
  }
  set {
    name  = "objectStore.region"
    value = var.region
  }
  set {
    name  = "objectStore.useIrsa"
    value = var.irsa_role_arn != "" ? "true" : "false"
  }
  set {
    name  = "env.ICEBERG_WAREHOUSE"
    value = "s3://${var.warm_bucket}"
  }

  dynamic "set" {
    for_each = var.irsa_role_arn == "" ? [] : [var.irsa_role_arn]
    content {
      name  = "serviceAccount.annotations.eks\\.amazonaws\\.com/role-arn"
      value = set.value
    }
  }
  set_sensitive {
    name  = "signingKey"
    value = var.signing_key
  }
}
