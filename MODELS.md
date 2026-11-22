# Model tiers

Tasks in `TASKS.md` carry a **tier**, not a model name. Model names change every few weeks;
tiers don't. Update the mapping table below when you change models; nothing else needs to change.

| Tier | Use for | Effort | Typical share of tasks |
| --- | --- | --- | --- |
| **A — deep** | Trust core (signing, chain, policy, auth, approvals), new interfaces other code builds on, injection defence, plan intake, milestone security review | High | ~30% |
| **B — standard** | Feature work that follows an existing pattern: a new source, a new MCP tool, workbench pages, Helm, Terraform, ML pipeline | Medium | ~50% |
| **C — fast** | Mechanical work: config, docs, CI wiring, fixtures, renames, lint and format fixes, test-failure triage with an obvious cause | Low / Fast | ~20% |

## Current mapping (edit me)

| Tier | Primary | Alternate | Notes |
| --- | --- | --- | --- |
| A | Candidates: Claude Fable, Claude Opus 5.5 (high effort) | _set after the bake-off_ | Prefer the strongest model you have for anything under `crates/attest-attestation`, `attest-policy-engine`, gateway authorization |
| B | Candidate: Claude Opus 5.5 (medium effort) | _set after the bake-off_ | |
| C | Candidates: a fast model (e.g. Grok 4.7 High Fast) or `Auto` | _set after the bake-off_ | Only if it passes T-004 unaided |
| Review (A) | A model from a **different vendor** than the one that wrote the code | | Cross-vendor review catches more than same-model review |

## Bake-off (do once, about an hour)

Pick the models by measurement, not reputation:

1. Create three branches from the same commit.
2. Run task **T-004** (small, fully testable) with the `attest-work-loop` skill on each candidate
   model, one per branch, in a fresh chat each.
3. Record for each: did `verify.sh full` pass without help, number of follow-up prompts you
   needed, tokens or cost shown by Cursor, and diff quality (read it).
4. Repeat with **T-012** for the tier-A candidates.
5. Fill the table above. Re-run when a new model appears.

## Token habits that matter more than the model choice

- **One task per chat.** Start a new agent chat for each task. Long chats re-send history every turn.
- **Point, don't paste.** Use `@TASKS.md`, `@<file>` and skills; skills load their detail only when used.
- **Keep context small.** Don't raise the context window unless the task needs a large file set.
- **Let `verify.sh quick` narrow the loop.** It only tests changed crates; run `full` once at the end.
- **The stop hook is capped** at 5 follow-ups (`ATTEST_HOOK_MAX_LOOPS`). If a task keeps
  failing, it should be marked `blocked: technical`, not retried forever.
- **Switch down for fix-ups.** After a tier-A model lands the design, use tier C for lint, docs
  and fixture fixes in the same branch.
