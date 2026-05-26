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

## Test through the production entry point

The acceptance test in your PR MUST exercise the same entry point
the user / MCP / smoke takes (MCP → controller → worker → core, or
CLI → core), not just the implementation-layer function you touched.
A green test against the layer you patched does not prove the fix
reaches production — the production path may bypass your layer
entirely.

Example: F-024 (see `rounds/round-04-2026-05-25/delta.md`,
"three-rebuild saga") burned three MCP rebuild cycles. Each
implementer's local test was real and green; each fix was real and
necessary. None moved the smoke needle alone because the production
MCP → viz-controller → viz-worker → core path bypassed the patched
layer until all three sites were fixed.

If reaching the production entry point from a unit/integration test
requires refactoring (extract a pure helper, bump `pub(crate)`
visibility for tests, etc.), do it in the same PR — that's in scope.

**Hard stop**: if testing through the production path is genuinely
infeasible (e.g. GUI thread plumbing can't be driven from a test),
flag it as a blocker on the finding and stop. Do not declare done on
an implementation-layer test alone.

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

## Regression-net rule (added by F-037)

PRs touching simulator-adjacent code MUST not regress the smoke baseline at
`planning/toolpath_acceptance/baselines/2026-05-26.csv`.

Relevant paths (any modification triggers this rule):

- `crates/rs_cam_core/src/{simulation_cut,tool_load,compute,toolpath,dexel*}/`
- `crates/rs_cam_core/src/{collision,gcode}.rs`
- `crates/rs_cam_viz/src/compute/`
- `crates/rs_cam_viz/src/controller/`

Procedure for a covered PR:

1. Re-run smoke locally with the runner the baseline was captured with:
   ```bash
   cargo run -p rs_cam_cli --release -- smoke --output /tmp/smoke_current.csv
   ```
2. Diff against the baseline:
   ```bash
   cargo run -p rs_cam_cli --release -- smoke --diff \
       --baseline planning/toolpath_acceptance/baselines/2026-05-26.csv \
       --output /tmp/smoke_current.csv
   ```
3. Exit code 0 (no regression) → attach `/tmp/smoke_current.csv` to the PR
   description with a one-liner ("smoke clean, 18 cases unchanged").
4. Exit code non-zero (regression detected) → either fix the regression OR
   justify it in the PR description with a cross-linked finding (e.g. F-031
   legitimately moved AS013 deflection 0.637 → 0.105 — a regression by the
   diff's lights but a real improvement). When a verdict change is
   legitimate, the same PR updates `baselines/<NEW DATE>.csv` and the path
   reference in this rule + the cargo regression test
   (`smoke_baseline_regression_f037.rs`).

The cargo test `smoke_baseline_regression_f037.rs` enforces the baseline-
file invariants (file present, parses cleanly, holds at least 5 deflection-
Within cases). It does NOT re-run the smoke (too slow for CI); the diff
above is the load-bearing check.

## Bonus: per-finding fix prompts

Some findings come with a prebuilt prompt at
`planning/acceptance_loop/rounds/round-NN-*/fix-prompts/PR-XXX-*.md`.
If one exists for your finding, use it; it has extra context the
finding file doesn't carry. If it doesn't exist, the finding file is
self-contained.
