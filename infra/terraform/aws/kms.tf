# Customer-managed key shared by the warm and anchor buckets. The key policy
# delegates to IAM (account root) so access is granted through the role
# policies in this module; nothing grants the key to a third party.
data "aws_iam_policy_document" "kms" {
  statement {
    sid       = "EnableIamPolicies"
    effect    = "Allow"
    actions   = ["kms:*"]
    resources = ["*"]
    principals {
      type        = "AWS"
      identifiers = ["arn:aws:iam::${data.aws_caller_identity.current.account_id}:root"]
    }
  }
}

resource "aws_kms_key" "attest" {
  description             = "Attest ${var.name} warm tier and attestation anchors"
  enable_key_rotation     = true
  deletion_window_in_days = var.kms_deletion_window_days
  policy                  = data.aws_iam_policy_document.kms.json
}

resource "aws_kms_alias" "attest" {
  name          = "alias/${var.name}"
  target_key_id = aws_kms_key.attest.key_id
}
