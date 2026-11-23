# Pluggable signer and AWS KMS

**Status:** Accepted

Attest signs every envelope with Ed25519 over `sha256` of the canonical body. The signer is now a trait. `LocalKey` is the in-process key, which is what `ATTEST_SIGNING_KEY` loads today. `AwsKms` calls AWS KMS and is compiled only with the `aws-kms` feature, so the default build has no AWS dependency.

## Algorithm decision

We checked the current AWS KMS docs (Nov 2026 — *Supported cryptographic algorithms* and the `Sign` API reference). KMS now signs Ed25519 with key spec `ECC_NIST_EDWARDS25519` and algorithm `ED25519_SHA_512` using `MessageType: RAW`, which is a standard RFC 8032 *pure* Ed25519 signature over the message. We send the 32-byte `sha256(canonical_body)` as that RAW message (well under the 4096-byte RAW limit). The result is byte-for-byte the signature `LocalKey` already produces, so both backends record `alg = ed25519` and share one keyring and one verifier. No new signature algorithm or crypto dependency enters the trust core.

Had KMS lacked Ed25519 (it did until recently), the fallback would be `ECDSA_SHA_256` on `ECC_NIST_P256` — the only KMS algorithm comparable to Ed25519 in size and speed — recorded as a distinct `alg`. The `SignatureAlg` enum and the alg-dispatch in `verify_with` exist so that fallback is a localized addition, not a rewrite. We did not invent an algorithm: the choice is encoded from the KMS `Sign` contract that `RecordedKmsClient` replays.

The envelope also records `key_id`. `alg` and `key_id` are not part of the signed body. They tell `attest verify` which public key to use; the signature itself binds the body to one private key, so neither field is forgeable into a valid envelope.

`attest verify --keyring <file>` trusts several keys. The file is JSON: `keys` is a list of `{ "key_id", "alg", "public_key" }`, and optional `legacy` verifies envelopes written before `key_id` existed. An envelope whose `key_id` is not in the file fails with `unknown key_id`. Envelopes signed before and after a rotation both verify when the file contains both keys.

The KMS client is a two-method trait (`Sign`, `GetPublicKey`). Tests use `RecordedKmsClient`, which signs with a local Ed25519 key and returns the public key in the same SPKI wrapper KMS returns. No test calls AWS.
