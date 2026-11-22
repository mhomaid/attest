---
name: attest-add-detection
description: Write or fix a HELIQL detection (or import a Sigma rule) with MITRE mapping and positive and negative fixture tests. Use when adding rules to detections/, porting a SIEM query to HELIQL, or fixing a rule that does not fire as described.
---

# Add a detection

Rules live in `detections/*.heliql`, parse with `crates/attest-heliql/src/grammar.pest`, and
compile to RisingWave materialized views (`compiler.rs`). Sigma YAML imports via `sigma.rs`.

## Steps

1. **State the behavior in one sentence**, including the threshold and window
   ("5 or more failed logins for one user from one IP within 10 minutes").
2. **Check the condition actually encodes that sentence.** Known bug to learn from:
   `okta_brute_force.heliql` says "repeated failed authentication" but its condition is
   `unique(event.cloud_region, window: 10m) > 0`, which fires on any single failure.
3. **Fixtures**: a positive event sequence that must fire and at least one negative sequence
   that must not (just under the threshold, or the benign look-alike). Synthetic only.
4. **Tests**: parse, compile, and evaluate against fixtures. If the evaluation harness does not
   exist yet (task T-004 adds it), add the rule's fixtures and a parse/compile test now and leave
   a note on T-004.
5. **Metadata**: `severity`, `mitre: [...]`, `runtime`, a description that matches the
   condition. Rules for a specific deployer belong in the private overlay; generic rules
   (brute force, impossible travel, OTP request bursts, card-testing patterns, WAF bypass
   spikes, config changes on security tooling, a detection that stopped running) belong here.
6. Run `attest-verify` (HELIQL area) and record evidence.
