---
name: attest-work-loop
description: Execute tasks from TASKS.md end to end (plan, failing test, implement, verify, record evidence, commit) and keep going until the task is proven done or truly blocked. Use for any implementation work in the Attest repo, or when asked to "continue", "do the next task", or "work through milestone M1".
---

# Attest work loop

You are not done when the code compiles. You are done when the task's acceptance criteria are
proven by tests, `verify.sh` is green, and the evidence is in `TASKS.md`.

## 0. Pick the task

1. Read `TASKS.md`.
2. If the user named a task ID or milestone, use it. Otherwise take the first task whose status
   is `todo` and whose `depends:` tasks are all `done`.
3. Set its status to `in_progress` and add today's date under **Evidence**.
4. Check its `model tier` (see `MODELS.md`). If you are running on a lower tier than the task
   asks for, tell the user in one line and continue only if they confirm.
5. If the scope covers several tasks (for example "do milestone M1"), loop through them in order
   without asking between tasks. Stop only under the rules in step 7.

## 1. Understand before you edit

- Read the task's acceptance criteria and the plan section it references in
  `docs/16_Production_Readiness_Plan.md`.
- Read the code it touches and the tests next to it. Use search, don't guess paths.
- Write a short plan (3 to 8 bullets) under the task in `TASKS.md` → **Plan**. Name the files
  you will change and the tests you will add.

## 2. Write the failing test first

- One test per acceptance criterion, at the lowest level that can prove it (unit, then crate
  integration, then e2e).
- For guards (auth, policy, verify, caps) always add a **negative** test: the bad input is refused.
- Fixtures are synthetic. See `attest-upstream-hygiene`.
- Run the test and confirm it fails for the right reason.

## 3. Implement

- Smallest change that makes the tests pass. Follow existing patterns in the crate.
- Use the specialised skills when they apply: `attest-add-source`, `attest-add-mcp-tool`,
  `attest-add-detection`.
- No new dependency without a one-line reason in the commit message.

## 4. Verify

```bash
.cursor/skills/attest-verify/scripts/verify.sh quick   # while iterating
.cursor/skills/attest-verify/scripts/verify.sh full    # before marking done
```

- If a check fails: read the output, fix the cause, run again. Never skip, `#[ignore]`, loosen an
  assertion or raise a latency budget to make it pass.
- Count attempts per failing check. After **5** failed attempts on the same check, go to step 7
  (blocked) with the error and what you tried.

## 5. Check hygiene and docs

```bash
.cursor/skills/attest-upstream-hygiene/scripts/check-org-leaks.sh
```

- Update the README Status table if a component's status changed.
- Add an ADR in `docs/` for any architectural choice (new dependency, new protocol, storage change).
- Update `env.example` and Helm `values.yaml` for any new setting.

## 6. Record and commit

Under the task in `TASKS.md`:
- **Evidence**: each acceptance criterion → the test name that proves it, plus the
  `verify.sh full` summary line and the date.
- If the task says `review: A`, set status to `review` and stop for that task: a tier-A model
  (ideally another vendor) or a person reviews the diff in a fresh chat with
  "Review T-xxx against its acceptance criteria and the trust-core rules". Fix what the review
  finds, then set `done`.
- Otherwise set status to `done`.

Commit: `git commit -s -m "<type>(<scope>): <summary> [T-xxx]"`. One task per commit where possible.

Then go back to step 0 if the scope has more tasks.

## 7. When to stop

Stop and report only when one of these is true:
- Every task in the requested scope is `done` with evidence.
- A task is `blocked: decision` — the acceptance criteria need a product or security decision.
  Write the question under the task, then continue with the next unblocked task.
- A task is `blocked: technical` after 5 attempts on one check. Write the error, what you tried,
  and your best hypothesis, then continue with the next unblocked task.
- You need a credential, account or paid service that is not available locally.

The final report lists: tasks done (with test names), tasks blocked (with the question),
and the `verify.sh full` result.

## Definition of done

See [references/definition-of-done.md](references/definition-of-done.md).
