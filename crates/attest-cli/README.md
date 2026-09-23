# `attest` CLI

Offline verify and replay for signed `AttestationEnvelope` logs. The binary
name is `attest`; the crate is `attest-cli`.

```sh
# Install from a checkout (uses the workspace Cargo.lock)
cargo install --path crates/attest-cli --locked

attest --help
attest verify --help
```

From a checkout without installing, put `--` before the subcommand so clap
sees it instead of cargo:

```sh
cargo run -q -p attest-cli -- verify LOG --key-file KEY
```

## Commands

### `attest verify LOG`

Recompute the canonical JSON hash, check the Ed25519 signature, walk
`prev_hash`, and reject duplicate `agent_action_id`.

| Flag | Also | Purpose |
|---|---|---|
| `--key-file PATH` | | Hex verifying key in a file (preferred) |
| `--key HEX` | `ATTEST_VERIFYING_KEY` | Hex on the command line / env |
| `--pin-model SHA256` | | Fail if `model_artifact_hash` differs |
| `--pin-prompt SHA256` | | Fail if LLM `system_prompt_hash` differs |

```sh
attest verify examples/verify/attestations.ndjson \
  --key-file examples/verify/verifying-key.txt
```

Copy the log before you break it. Do not `sed -i` the file in git.

```sh
sed '2d' examples/verify/attestations.ndjson > /tmp/broken.ndjson
attest verify /tmp/broken.ndjson --key-file examples/verify/verifying-key.txt
# chain break → exit 1
```

### `attest replay ACTION_ID --log LOG`

Classifier / hybrid / auto-close envelopes re-run the ONNX model on the
recorded features (`raw_prediction` within `1e-6`, stored verdict).
LLM envelopes are integrity-only: signature, `system_prompt_hash` present,
tool-call timestamps in order. The model output is not regenerated.

```sh
attest replay "$ACTION_ID" \
  --log ./attestations.ndjson \
  --key-file ./verifying-key.txt \
  --model ml/triager/artifacts/model.onnx
```

`--model` is required for a classifier envelope.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Every envelope passed |
| 1 | At least one envelope failed, or a usage / I/O error |

## Key handling

Prefer `--key-file` or `ATTEST_VERIFYING_KEY` over `--key HEX`. A hex flag
shows up in `ps` and often in shell history.

The orchestrator prints the verifying key at startup. If
`ATTEST_SIGNING_KEY` is unset it generates an ephemeral key — fine locally,
useless after a restart because older envelopes will not verify.

Regenerate the committed sample log with:

```sh
cargo run -q -p attest-cli --example make_sample
```
