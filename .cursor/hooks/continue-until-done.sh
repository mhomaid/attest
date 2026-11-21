#!/usr/bin/env bash
# Cursor `stop` hook: if the agent stops while a task in TASKS.md is still `in_progress`,
# send one follow-up asking it to finish the work loop (verify, record evidence, mark done
# or blocked). Capped by Cursor's loop_count so it can never spin forever.
#
# stdout must be JSON only. Logs go to stderr.

set -uo pipefail
MAX_LOOPS="${ATTEST_HOOK_MAX_LOOPS:-5}"

input="$(cat)"
status="$(printf '%s' "$input" | sed -n 's/.*"status"[[:space:]]*:[[:space:]]*"\([a-z_]*\)".*/\1/p' | head -1)"
loops="$(printf '%s' "$input" | sed -n 's/.*"loop_count"[[:space:]]*:[[:space:]]*\([0-9]*\).*/\1/p' | head -1)"
loops="${loops:-0}"

# Only continue after a normal completion; respect user aborts and errors.
if [[ "$status" != "completed" ]] || (( loops >= MAX_LOOPS )); then
  echo '{}'
  exit 0
fi

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
TASKS="$ROOT/TASKS.md"
[[ -f "$TASKS" ]] || { echo '{}'; exit 0; }

# Find a task heading followed by `- status: in_progress`
open_task="$(awk '
  /^### T-[0-9]+/ { id=$2 }
  /^- status:[[:space:]]*in_progress/ { if (id != "") { print id; exit } }
' "$TASKS")"

if [[ -z "$open_task" ]]; then
  echo '{}'
  exit 0
fi

echo "continue-until-done: $open_task still in_progress (loop $loops/$MAX_LOOPS)" >&2
msg="Task $open_task is still in_progress in TASKS.md. Continue with the attest-work-loop skill: run .cursor/skills/attest-verify/scripts/verify.sh full, fix any failure, write the evidence under the task, then set it to done, or to blocked with the reason. Do not stop while it is in_progress."
printf '{"followup_message": "%s"}\n' "$msg"
