# Pluggable signer and AWS KMS

**Status:** Accepted

Attest signs every envelope with Ed25519 over `sha256` of the canonical body. The signer is now a trait. `LocalKey` is the in-process key, which is what `ATTEST_SIGNING_KEY` loads today. `AwsKms` calls AWS KMS and is compiled only with the `aws-kms` feature, so the default build has no AWS dependency.

AWS KMS signs Ed25519 with key spec `ECC_NIST_EDWARDS25519` and algorithm `ED25519_SHA_512`. That signature is the same RFC 8032 signature `LocalKey` produces, so both backends record `alg = ed25519`. The envelope also records `key_id`. Those two fields are not part of the signed body. They tell `attest verify` which public key to use.

`attest verify --keyring <file>` trusts several keys. The file is JSON: `keys` is a list of `{ "key_id", "alg", "public_key" }`, and optional `legacy` verifies envelopes written before `key_id` existed. An envelope whose `key_id` is not in the file fails with `unknown key_id`. Envelopes signed before and after a rotation both verify when the file contains both keys.

The KMS client is a two-method trait (`Sign`, `GetPublicKey`). Tests use `RecordedKmsClient`, which signs with a local Ed25519 key and returns the public key in the same SPKI wrapper KMS returns. No test calls AWS.
