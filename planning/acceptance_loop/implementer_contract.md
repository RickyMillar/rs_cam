# Implementer contract

You are an implementer in the acceptance loop. Your job: land one
finding as one PR, with the acceptance test from the finding file
included as a regression test. You do **not** run the smoke acceptance
suite or claim "verified" — that's the auditor's job in the next round.

Read `README.md` first if unfamiliar with the loop.

## Read these in order

1. `planning/acceptance_loop/STATE.md` — for the queue and any
   round-specific notes
2. `planning/acceptance_loop/findings/F-XXX-*.md` — the specific
   finding you're claiming
3. The repo `CLAUDE.md` — for lint policy, dev workflow, MCP context
4. `planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md` — if your fix touches
   an acceptance bar, so you know what verdict shape it must produce

## Picking a finding

Take the **top unclaimed** finding from STATE.md's queue. Don't cherry-
pick. Don't batch multiple findings into one PR unless STATE.md
explicitly says they should land together (e.g. F-012 is a duplicate
of F-003 and closes with it).

If the top finding has `Status: in_flight` already (someone else
claimed it), drop to the next unclaimed one. Don't race.

## Claim the finding

Update `STATE.md` "In flight" table:

```
| F-XXX | <your agent name or session id> | (PR pending) | <one-line note> |
```

Update the finding file's frontmatter:

```
Status: in_flight (claimed by <agent>, <YYYY-MM-DD>)
```

Commit message convention: include the finding ID in the title.

```
fix(F-XXX): <one-line summary> — closes F-XXX
```

## Write the fix

Constraints:

- One finding → one PR. No drive-by refactors. If you find another
  bug while fixing yours, open a new finding (just write the file —
  the auditor will sort the queue).
- Acceptance test from the finding file MUST be included in the PR.
  If the finding file's acceptance test is vague, sharpen it in the
  same PR — but the auditor must be able to point at concrete test
  names that pass on this PR and fail on master.
- Pass `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test -q` before declaring done.
- If `cargo test --test param_sweep` fingerprints change because of
  your fix, regenerate them and commit the regen as part of the same
  PR with a note. Don't fight the fingerprint suite.
- Don't touch unrelated files. Use `git diff --stat` to sanity-check
  the PR's footprint matches the finding's effort estimate (±30%).
- Don't bypass lints (`#[allow]`) unless the finding explicitly
  authorizes it. If you must, add a `// SAFETY:` comment per the
  CLAUDE.md lint policy.

## Don'ts

- **Don't** mark the finding `landed` until the PR is merged.
  `in_flight` means "PR is open and I'm working on it".
- **Don't** mark anything `verified`. Only the auditor's next smoke
  run does that.
- **Don't** edit other findings' files. If you have evidence one of
  them is wrong or duplicates yours, leave a one-line note in your PR
  description; auditor reconciles.
- **Don't** edit the smoke datasets (`cases_agent_smoke.csv`,
  `seed_goal_matrix.csv`, `toolpath_catalog.csv`) without auditor
  approval. Those are calibration ground truth.
- **Don't** run the acceptance smoke suite to "check your work." Smoke
  is a round-level signal, not a per-PR signal. Your unit / integration
  test is what proves the PR is correct.
- **Don't** delete or rename finding files.

## When the PR is ready

1. Update the finding file's frontmatter:

```
Status: landed
Linked PRs: #NNN
```

2. Append to STATE.md "Implementation log":

```
- 2026-XX-XX — F-XXX landed: <one-line summary>. PR #NNN. Acceptance test: <test name>.
```

3. Remove your row from STATE.md "In flight" table.

4. Stop. Don't claim verified. Don't run the smoke. Don't pick up the
   next finding unless STATE.md's queue still has unclaimed work AND
   the user said to keep going.

## When to stop and ask the user

- The finding's acceptance test, as written, can't be expressed as a
  unit/integration test (means it's actually a smoke-level finding —
  ask the auditor to re-frame it)
- The fix turns out to be larger than the finding's effort estimate
  by more than 2×
- The fix would require also touching the "What NOT to touch" list in
  `CODEBASE_UNIFICATION_PLAN.md` (until that doc is fully migrated
  into findings, treat its exclusions as authoritative)
- You find a finding that's duplicated or contradicted by another
  finding file — let the auditor reconcile

## Bonus: per-finding fix prompts

Some findings come with a prebuilt prompt at
`planning/acceptance_loop/rounds/round-NN-*/fix-prompts/PR-XXX-*.md`.
If one exists for your finding, use it; it has extra context the
finding file doesn't carry. If it doesn't exist, the finding file is
self-contained.
