# Chain-tip anchors

**Status:** Accepted

The hash chain detects a deleted or reordered middle row. It does not detect someone deleting the last rows and the tip pointer together. An anchor fixes that.

On a timer (`ATTEST_ANCHOR_EVERY_SECS`, default 300) the orchestrator writes a signed object:

```json
{ "seq": 1200, "tip_hash": "...", "signed_at": "...", "key_id": "...", "alg": "ed25519", "signature": "..." }
```

The object key is `{prefix}/seq-{seq}.json`. Put that prefix on a bucket with S3 Object Lock in **compliance** mode and a retention period longer than your audit window. Compliance mode means even the account root cannot delete or shorten the object before retention expires. Enable it when the bucket is created; it cannot be turned on later for existing objects in a way that covers history you already lost.

`attest verify LOG --key-file KEY --anchors anchor.json` fails with `truncated after seq N` when the log has fewer rows than the anchor's `seq`.

`ATTEST_ANCHOR_WEBHOOK` is empty by default. Set it to a URL only if an external channel should receive a copy of each anchor. Nothing is sent when the variable is unset.
