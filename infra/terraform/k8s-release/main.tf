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
    value = var.object_store_region
  }
  set {
    name  = "objectStore.useIrsa"
    value = var.use_irsa ? "true" : "false"
  }
  set {
    name  = "env.ICEBERG_WAREHOUSE"
    value = "s3://${var.warm_bucket}"
  }

  dynamic "set" {
    for_each = var.object_store_endpoint == "" ? [] : [var.object_store_endpoint]
    content {
      name  = "objectStore.endpoint"
      value = set.value
    }
  }

  dynamic "set_sensitive" {
    for_each = var.object_store_access_key == "" ? [] : [var.object_store_access_key]
    content {
      name  = "objectStore.accessKey"
      value = set_sensitive.value
    }
  }

  dynamic "set_sensitive" {
    for_each = var.object_store_secret_key == "" ? [] : [var.object_store_secret_key]
    content {
      name  = "objectStore.secretKey"
      value = set_sensitive.value
    }
  }

  dynamic "set" {
    for_each = var.service_account_annotations
    content {
      name  = "serviceAccount.annotations.${replace(set.key, ".", "\\.")}"
      value = set.value
    }
  }

  set_sensitive {
    name  = "signingKey"
    value = var.signing_key
  }
}
