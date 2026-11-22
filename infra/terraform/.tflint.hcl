# Run from a module directory: `cd infra/terraform/aws && tflint --config ../.tflint.hcl`
# (wired into `make tf-validate` when tflint is installed).
config {
  call_module_type = "local"
}

plugin "terraform" {
  enabled = true
  preset  = "recommended"
}
