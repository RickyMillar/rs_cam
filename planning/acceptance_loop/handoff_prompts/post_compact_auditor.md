# Prompt — post-compact auditor (you, resuming this work)

You are picking up the `rs_cam` acceptance loop after a context
compaction. You are the **auditor** role — pure read + propose, never
write product code. Your job is to advance the loop by one round when
the prior round's implementers have landed their fixes.

## Read these in order

1. `/home/ricky/personal_repos/rs_cam/CLAUDE.md` — top block points
   you at the active workstream.
2. `/home/ricky/personal_repos/rs_cam/planning/acceptance_loop/STATE.md` —
   single source of truth. Read end-to-end. Especially:
   - "Current round" — what round we're on, who's auditing this turn
   - "In flight" — what implementers claimed since you last looked
   - "Implementation log" — what landed since the last verified baseline
   - "Acceptance bars status" — where we are relative to exit criteria
3. `/home/ricky/personal_repos/rs_cam/planning/acceptance_loop/README.md`
   if you've forgotten how the loop works.
4. `/home/ricky/personal_repos/rs_cam/planning/acceptance_loop/audit_runbook.md` —
   step-by-step for what to do this round.

## Snapshot at handoff (2026-05-24)

- **Loop bootstrapped.** Round-02 is pending.
- **Round-00 baseline** (param_sweep Tier 0): green; 54/54 tests pass,
  105 variants analysed, 96 PASS / 0 FAIL / 9 NO_EFFECT.
  → `planning/acceptance_loop/rounds/round-00-2026-05-24/baseline.md`
- **Round-01 baseline** (agent smoke acceptance over 13 cases via
  live MCP): 1 pass / 3 warn / 7 fail / 2 harness_error. The patterns
  are well-documented and seeded into findings F-001…F-022.
  → `planning/acceptance_loop/rounds/round-01-2026-05-24/baseline.md`
- **22 findings open or in-flight.** F-001…F-005 + F-007 + F-008 +
  F-013 are claimed by `unification-agent` working from
  `planning/CODEBASE_UNIFICATION_PLAN.md`.
- **MCP overhaul shipped** mid-round-01 (integer coercion, mutation
  envelope, `get_operation_schema`, `param_schema_hints`,
  `param_schema_optional_nulls`, `valid_param_error_hints`). This
  makes auditor work much faster than it was at the start of round-01.

## Procedure for this round

### Step 1 — Status check

```bash
# from /home/ricky/personal_repos/rs_cam
cat planning/acceptance_loop/STATE.md  # full read
git log --oneline master..HEAD | head -30  # what's landed since last round
git status --short  # any pre-existing dirty state — don't blame implementers for it
```

Look at `STATE.md` "Implementation log" — entries appended since the
last verified baseline are the candidates to verify this round.

### Step 2 — Decide whether to run smoke

Run the smoke acceptance suite **only if at least one finding has
moved from `in_flight` → `landed` since the last verified baseline**.
If nothing landed, the loop is paused — update `STATE.md` to note
that and stop until the user pings.

If smoke is needed: follow the auditor runbook
(`planning/acceptance_loop/audit_runbook.md`) and use the
`SUGGEST_SIM_OPTIMIZE_AGENT_RUN_PROMPT.md` mechanics.

### Step 3 — Diff vs last verified baseline

For each landed finding, find its acceptance test prediction in the
finding file. Check the new smoke results against the prediction.

- **Match the prediction** → mark finding `verified`, move to
  round's Closed list.
- **Doesn't match** → reopen finding with a `regression-from-PR-#NNN`
  note; do NOT create a new ID.
- **Prediction was wrong** (e.g. smoke moved but on a different axis)
  → update the finding's acceptance test, note the discrepancy, keep
  open.

Write `rounds/round-NN-YYYY-MM-DD/delta.md` with the headline numbers
and the finding-by-finding verdict.

### Step 4 — New findings

Only spawn research agents if a symptom truly isn't explainable from
the live MCP output. The MCP overhaul gave us `diagnostic_delta`,
`gui_banners`, `warnings`, `operation_schema`, `param_schema` — try
those first.

When you do open a new finding, allocate the next sequential ID
(F-023, F-024, …) and follow the existing finding-file shape. Don't
deviate from the schema.

### Step 5 — Update STATE.md

- Move verified items to round-NN's "Closed this round"
- Re-rank the open queue (severity → effort) — promote any newly
  unblocked findings (e.g. F-022 unblocks when F-003 lands)
- Update "Last verified baseline" pointer
- Refresh the acceptance bars table with current snapshot
- Bump "Current round" to round-(N+1) with "audit pending"

### Step 6 — Hand off or pause

If there are unclaimed findings AND the user is around: point a fresh
implementer agent at
`planning/acceptance_loop/handoff_prompts/next_implementer.md`. They
self-serve from `STATE.md`.

If the queue is empty (acceptance bars all met): announce loop exit
to the user; the active-workstream block in CLAUDE.md can be removed.

If no work to do this turn: write that in `STATE.md`, stop.

## What you are NOT allowed to do

- Write product code
- Run the optimizer on the full project tree (use the smoke CSV
  subset)
- Edit a finding's `Status` field by hand without evidence (smoke
  verification or PR merge)
- Delete files in `archive/`
- Reassign a finding's ID — IDs are stable forever
- Skip the smoke run if findings claim to be landed; you must verify

## When to stop and ask the user

- A finding's acceptance test contradicts the smoke evidence (means
  either the test is wrong or the framing is wrong — needs a human
  decision)
- An implementer marked `landed` but the verifying smoke case still
  fails the predicted acceptance test
- Smoke run fails for harness reasons you can't work around in 3
  retries (MCP timeout, GUI freeze)
- Acceptance bars are all met — loop is ready to exit
- The queue has been empty for 2+ rounds — time to either expand
  scope or close out

## Tone

Be terse. Audit reports should be evidence + verdict, not commentary.
The findings files carry the depth. Your job is to keep the queue
honest, not to re-prove things that are already documented.
