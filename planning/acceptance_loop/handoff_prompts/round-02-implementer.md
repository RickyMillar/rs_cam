# Prompt — next implementer (round-02 pickup, F-015)

You are an implementer in the `rs_cam` acceptance loop. The round-01
unification batch + F-016 just landed on master (commits 072c11a +
2a287c1). The post-compact auditor has test-verified those at the
unit/integration level but cannot run live MCP smoke right now, so
your fix will roll into the next auditor round's combined
verification.

Your job is to land **one finding as one PR**, with the acceptance
test from the finding file included as a regression test. You do not
run the smoke acceptance suite. You do not claim "verified". That is
the next auditor's job in round-03.

## Pickup: F-015

**File:** `planning/acceptance_loop/findings/F-015-op-precondition-static-validation.md`

**Summary:** Op-precondition static validation rules are missing for
rest machining, drill, and project_curve. Today these fail only at
generate-time with a runtime error (AS006 in the round-01 smoke hit
"Error: Rest machining requires an earlier enabled operation"). They
should surface as a static validator diagnostic on the Params tab so
the user sees the problem before clicking Generate.

**Why it's at the top of the queue:** medium severity, M effort,
independent of every in-flight finding, and unblocks AS006 in the
next smoke run — which is one of the cases that will measure whether
the round-01 unification batch actually moved the bars.

## Read these in order (skim is fine, but read all)

1. `/home/ricky/personal_repos/rs_cam/CLAUDE.md` — top block points
   you at the active workstream. Lint policy is strict (zero warnings).
2. `/home/ricky/personal_repos/rs_cam/planning/acceptance_loop/STATE.md` —
   single source of truth. Verify F-015 is still at the top before
   claiming it; the queue may have changed between this prompt and
   when you start.
3. `/home/ricky/personal_repos/rs_cam/planning/acceptance_loop/implementer_contract.md` —
   your contract. The "Don'ts" section is short; read it.
4. `/home/ricky/personal_repos/rs_cam/planning/acceptance_loop/findings/F-015-op-precondition-static-validation.md` —
   the finding itself: evidence, acceptance test, files, fix shape, risk.
5. `/home/ricky/personal_repos/rs_cam/planning/acceptance_loop/handoff_prompts/next_implementer.md` —
   the generic implementer guide. Read it for the procedure boilerplate
   so this prompt can stay focused on F-015.

## Procedure (mirrors `next_implementer.md` — quick recap)

1. **Claim** — move F-015's row from "Open queue" to "In flight" in
   STATE.md with your row: `| F-015 | <agent name> | (PR pending) |
   <one-line note> |`. Update the finding file's `Status:` to
   `in_flight (claimed by <agent>, 2026-05-25)`.
2. **Implement** — add the static validation rules; surface them via
   whatever static-validation diagnostic adapter the codebase already
   uses (it shipped in PR-2B / PR-2C — see commit 8ac3720 for
   `feat(validate): A1 project_curve depth-sign + B1 runtime confirm`,
   that's the pattern to follow). Include the acceptance test from
   the finding file as a regression test.
3. **Gate** — `cargo clippy --workspace --all-targets -- -D warnings`
   + `cargo test -q` both clean before declaring done.
4. **Hand off** — finding `Status: landed`, append to STATE.md
   Implementation log, remove your "In flight" row, stop. Do NOT
   pre-populate "Closed this round" — that is the next auditor's call.

## Hints from the round-01 smoke evidence

- The pattern from PR-2B (project_curve depth-sign) is the model:
  static validation surfaces in the Params tab banner, runtime
  confirm gates the operation. Mirror that shape for rest / drill /
  project_curve preconditions.
- Read AS006's "Error: Rest machining requires an earlier enabled
  operation" in `planning/acceptance_loop/rounds/round-01-2026-05-24/baseline.md`
  for the exact message users see today and the case that exercises it.
- Three preconditions to surface (see finding file for definitive list):
  - **Rest machining**: requires at least one prior enabled operation in
    the setup that produced material removal.
  - **Drill**: requires hole positions (model centroids or stock
    `alignment_pins`).
  - **project_curve**: requires a source curve in the scene.

## Don'ts

- Don't batch multiple findings into one PR (the unification batch was
  a one-time exception).
- Don't run the smoke acceptance suite to "check your work" — that is
  a round-level signal.
- Don't mark anything `verified`. Only the auditor does that, and only
  with live smoke evidence.
- Don't edit other findings' files.
- Don't reset/stash/revert pre-existing worktree state.

That's it. Claim F-015, implement, gate, hand off, stop.
