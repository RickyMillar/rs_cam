# Prompt — next implementer agent

You are an implementer in the `rs_cam` acceptance loop. Your job is
to land **one finding as one PR**, with the acceptance test from the
finding file included as a regression test. You do not run the smoke
acceptance suite. You do not claim "verified". That's the auditor's
job in the next round.

## Read these in order (skim is fine, but read all four)

1. `/home/ricky/personal_repos/rs_cam/CLAUDE.md` — top block points
   here; the rest covers lint policy, dev workflow, MCP context.
2. `/home/ricky/personal_repos/rs_cam/planning/acceptance_loop/STATE.md` —
   current round, what's claimed, what's in the queue. **This is the
   single source of truth.** Always read first; always update at the end.
3. `/home/ricky/personal_repos/rs_cam/planning/acceptance_loop/implementer_contract.md` —
   your contract. Read the "Don'ts" section carefully.
4. `/home/ricky/personal_repos/rs_cam/planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md` —
   the acceptance bars your work will eventually be measured against.
   Read so you know what verdict shape your fix needs to produce.

## Procedure

### Step 1 — Pick a finding

Find the **top unclaimed** finding in `STATE.md`'s queue. As of the
loop bootstrap (round-02 pending), several findings are already
`in_flight` claimed by `unification-agent` (F-001, F-002, F-003,
F-007, F-008, F-013). Drop past those to the next unclaimed.

The most likely candidates for the first independent implementer
(not the unification-batch agent) are:

- **F-015** — Op-precondition static validation (rest, drill,
  project_curve) — medium severity, M effort, unblocks AS006 in the
  smoke suite, fully independent of the unification batch.
- **F-016** — Drill `chip_welding` material threshold — medium
  severity, S effort, one-liner likely.
- **F-018** — Test data templates don't match smoke CSV — medium
  severity, S effort, pure fixture work.

Pick one. Don't batch.

### Step 2 — Claim it

Update `STATE.md`:

- Move the finding's row in the queue to the "In flight" table
- Add your row: `| F-XXX | <your agent name or session id> | (PR pending) | <one-line note> |`

Update the finding file's frontmatter (`Status:` field):

```
Status: in_flight (claimed by <agent>, <YYYY-MM-DD>)
```

### Step 3 — Implement

- One finding → one PR. No drive-by refactors.
- Include the acceptance test from the finding file. The auditor
  must be able to point at concrete test names that pass on your
  branch and fail on master.
- Pass `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test -q` before declaring done.
- Lint policy in CLAUDE.md is strict (zero warnings). Read it.
- Follow the workspace conventions (tests live next to code, etc.).
- Commit message: `fix(F-XXX): <one-line summary> — closes F-XXX`

### Step 4 — Hand off

When the PR is merged (or ready for review, if you're not authorised
to merge):

1. Update finding file frontmatter:
   ```
   Status: landed
   Linked PRs: #NNN
   ```
2. Append one line to `STATE.md` "Implementation log":
   ```
   - YYYY-MM-DD — F-XXX landed: <one-line summary>. PR #NNN. Acceptance test: <test name>.
   ```
3. Remove your row from `STATE.md` "In flight" table.
4. **Stop.** Do not claim verified. Do not run the smoke. Do not pick
   up the next finding unless the user explicitly says to keep going.

## When to stop and ask the user

- The finding's acceptance test, as written, can't be expressed as a
  unit/integration test (means it's a smoke-level finding — ask the
  auditor to re-frame)
- The fix turns out larger than the finding's effort estimate by > 2×
- The fix requires touching the "What NOT to touch" list from
  `planning/CODEBASE_UNIFICATION_PLAN.md` or violates an exclusion
  in `CLAUDE.md`
- Another finding contradicts or duplicates yours — let the auditor
  reconcile

## Don'ts (from the implementer contract — re-read it)

- Don't mark anything `verified`. Auditor only.
- Don't edit other findings' files
- Don't edit the smoke datasets without auditor approval
- Don't run the smoke acceptance suite to "check your work" — that's
  a round-level signal, not a per-PR signal
- Don't delete or rename finding files; IDs are stable forever
- Don't reset / stash / revert pre-existing worktree state

That's it. Pick a finding. Land the PR. Update STATE.md. Stop.
