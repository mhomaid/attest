output "release" {
  value = helm_release.attest.name
}

output "namespace" {
  value = helm_release.attest.namespace
}
