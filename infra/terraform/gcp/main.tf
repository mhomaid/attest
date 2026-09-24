data "google_project" "this" {}

locals {
  bucket = "${var.name}-warm-${data.google_project.this.number}"
  wi     = var.k8s_namespace != ""
}

resource "google_storage_bucket" "warm" {
  name                        = local.bucket
  location                    = var.location
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"
  force_destroy               = var.force_destroy

  versioning {
    enabled = true
  }

  lifecycle_rule {
    condition {
      age = 7
    }
    action {
      type = "AbortIncompleteMultipartUpload"
    }
  }

  labels = {
    product = "attest"
    tier    = "iceberg-warm"
  }
}

# HMAC keys speak the S3 API (storage.googleapis.com). That is what
# attest-storage-iceberg uses today (AmazonS3Builder).
resource "google_service_account" "warm" {
  account_id   = "${var.name}-warm"
  display_name = "Attest Iceberg warm tier"
}

resource "google_storage_bucket_iam_member" "warm" {
  bucket = google_storage_bucket.warm.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${google_service_account.warm.email}"
}

resource "google_storage_hmac_key" "warm" {
  service_account_email = google_service_account.warm.email
}

resource "google_service_account_iam_member" "workload_identity" {
  count              = local.wi ? 1 : 0
  service_account_id = google_service_account.warm.name
  role               = "roles/iam.workloadIdentityUser"
  member             = "serviceAccount:${var.project}.svc.id.goog[${var.k8s_namespace}/${var.k8s_service_account}]"
}
