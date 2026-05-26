# F-030 — Architecture refactor brief (fresh session)

**Use this prompt to drop into a new Claude session and have it land
F-030 without depending on the audit session's context.**

## Heads up — concurrent audit session active (2026-05-25)

The acceptance-loop auditor session that opened F-030 is still running
in another Claude session. Expect to see additive edits land while you
work, in **`planning/acceptance_loop/`** only — never in `crates/`:

- `STATE.md` (queue rerank, implementation log appends)
- `rounds/round-07-2026-05-25/delta.md` (new file, being written up)
- Possibly new findings if the auditor's round-07 sweep surfaces more
  (e.g. F-029 reframe, additional deferred-case observations)
- Possibly the F-028 finding frontmatter (closure notes)

These are disjoint from F-030's scope (`crates/rs_cam_core/src/session/`,
`crates/rs_cam_viz/src/{compute,controller}/`). **If you see a git
merge / rebase prompt or unexpected dirty state in `planning/`, do not
revert — those are audit edits.** Treat them as readable context, not
your turf.

If the auditor lands a new finding that supersedes F-030 (unlikely
within a single round, but flag if it happens), **stop and ask the user**
rather than racing.

If you need to verify the audit session's latest understanding before
designing, the freshest source is `STATE.md` "Implementation log" tail
and `rounds/round-07-2026-05-25/delta.md` (when written).

You're the implementer for **F-030 — Unify SetupEvalContext across the
5 stock-frame entry points** in the `rs_cam` acceptance loop. This is
**not a point fix**; it's an architectural refactor to retire a class
of bug that has surfaced in 4 separate findings (F-024, F-026, F-027,
F-028) and is expected to continue surfacing in F-025 and F-029 if not
addressed.

## Read in order (self-contained)

1. **`planning/acceptance_loop/findings/F-030-unify-setup-eval-context.md`**
   — the finding. Has the 5 sites, the proposed abstraction
   (`SetupEvalContext`), constraints, side-effect wins, and the
   acceptance criterion.

2. **`planning/acceptance_loop/implementer_contract.md`** — your job
   description. Especially the "Test through the production entry
   point" section.

3. **`planning/acceptance_loop/rounds/round-04-2026-05-25/delta.md`**
   "three-rebuild saga" and **`rounds/round-07-2026-05-25/delta.md`**
   (the F-028 viz-path follow-up) — these are the precedents that
   prove the pattern. Read both before designing.

4. **`CLAUDE.md`** — workspace lint policy and CAM context.

5. The five sites listed in F-030's table. Read each one — most are
   short.

## Preconditions (check before starting)

- The acceptance loop's deflection bar must be at 7/7 Within on
  AS001-AS006 + AS013 + AS015 + AS004 in the most recent round delta.
  If the bar is not there (e.g. F-029 hasn't landed), **stop and ask
  the user** — F-030 should not land against a moving deflection
  baseline. The acceptance suite is the refactor's only safety net.
- Check `git log --oneline -10` — confirm F-028's viz follow-up
  commit `c9e203d` is in the log. If a newer commit family has
  reshuffled the 5 sites, re-enumerate before starting (the F-030
  finding's table may be stale).
- `pgrep -af cargo` — confirm no concurrent cargo build (the user
  occasionally builds `rs_cam_viz` release; per
  `feedback_no_concurrent_release_builds.md` memory, don't run cargo
  test during release builds).

## Step 1 — Enumerate the actual site list

The F-030 finding lists 5 sites I'm aware of. Run these greps to
confirm completeness — there may be more (CLI sim path, export gate,
GCode emission, dressup pipelines):

```bash
rg "effective_stock_bbox|stock_bbox\(\)" --type rust crates/
rg "face_up.*=.*FaceUp::Top|z_rotation.*=.*Rotation::Deg0" --type rust crates/
rg "local_to_global|transform_setup|needs_transform" --type rust crates/
rg "auto_from_model" --type rust crates/
rg "HeightContext|HeightsConfig::resolve" --type rust crates/
```

Build a definitive site list. Post it as a comment on the finding (or
as the PR description's "sites touched" list) before writing code.

If the list grows past ~7 sites, **stop and ask the user** — the
effort estimate (L) may be wrong, or the refactor needs splitting.

## Step 2 — Design the abstraction

F-030 sketches `SetupEvalContext`. Refine based on the actual site
inventory. The key invariants:

- **Single source of truth**: world stock bbox derived once
  (honoring `stock.origin_*` + `auto_from_model` re-derivation from
  model union).
- **Identity-or-not encoded once**: a `None` for the transform means
  identity; a `Some(transform)` carries the full local↔global
  matrix. F-028's `.filter(|s| s.needs_transform())` pattern is the
  seed.
- **Operations consume `&SetupEvalContext`**: they no longer touch
  `session.stock_config()` or `session.stock_bbox()` directly. If
  an operation needs material, padding, workholding — those go in
  `SetupEvalContext::stock_meta`.
- **F-024's safe_z floor**: preserve the "effective_safe_z reads from
  local bbox to floor at a conservatively-higher value, never below
  world stock top" behavior. See F-028 commit `e48d7df` body.

Write a one-page design (markdown, top of your PR description, no
separate file) before coding. Include the struct definition + the
"how it threads" diagram + the migration list (site by site).

## Step 3 — Implement atomically

**One PR.** Don't split into "introduce context, then migrate sites" —
a half-migrated tree is worse than the current state because two
incompatible conventions exist simultaneously.

Order within the PR:
1. Define `SetupEvalContext` in `crates/rs_cam_core/src/session/` (or
   a new module if it spans both crates).
2. Add `impl SetupEvalContext::build(&ProjectSession, setup_index)`.
3. Migrate site 1 (load), then sites 2-5 (compute / viz worker / viz
   controller sim / viz controller gen). After each site is
   migrated, run the corresponding test file
   (`_f024.rs`, `_f026.rs`, `_f027.rs`, `_f028.rs`) — they must
   still pass.
4. Delete the now-dead helper functions (`build_world_stock_bbox`
   etc) if the migration absorbed them. If they're still needed for
   tests, keep them but reduce them to one-line shims.
5. Run `cargo clippy --workspace --all-targets -- -D warnings` and
   `cargo test -q` — both must pass.

## Step 4 — Acceptance

**Pass criterion**: every existing `_f024.rs`, `_f026.rs`, `_f027.rs`,
`_f028.rs` test passes. **No new failure of any existing acceptance
test.** If you find a test that was previously passing trivially (per
the F-028 follow-up loophole — `peak_axial <= 0.6` trivially satisfied
by 0), tighten it during the refactor. Don't loosen any test to make
the refactor pass.

Add a new test only if the refactor exposes a contract not yet
covered (e.g. an enumeration of all `SetupEvalContext::build` inputs
that should round-trip identically).

## Step 5 — Hand back to the auditor

When `cargo test -q` passes:
1. Commit with title `refactor(F-030): unify SetupEvalContext across stock-frame entry points`.
2. Update `STATE.md` "Implementation log" with the commit SHA and a
   summary listing the sites collapsed.
3. Update the F-030 finding's frontmatter:
   `Status: landed`. Add the commit to `Linked PRs`.
4. **Tell the user `/mcp` rebuild needed** — the running MCP server
   embeds the pre-refactor code; the auditor cannot verify until
   rebuild.
5. Stop. Do NOT run the smoke acceptance suite. That's the auditor's
   job in round-09 (or whatever round comes next).

## Hard rules

- **One PR.** No drive-by fixes.
- **Don't touch smoke datasets** (`planning/toolpath_acceptance/cases_agent_smoke.csv` etc).
- **Don't bypass lints** without a `// SAFETY:` comment (per CLAUDE.md).
- **Don't claim verified.** Only the auditor's smoke run verifies.
- **Don't bundle F-029 or F-025.** They're separate findings. If
  your refactor incidentally fixes them at the simulator level,
  mention in the PR description but do not delete the finding files
  — auditor reconciles.
- **Don't `--no-verify` or `--no-gpg-sign`**.
- **Don't force-push.** Create a new commit if you need to amend.
- **Don't worktree-extract**. Per
  `feedback_worktree_extraction.md` memory, refactors touching
  recently-modified files break in worktree isolation. Stay in main.

## What you NEVER do autonomously

- Mark anything `verified` — that's smoke evidence only.
- Edit other findings' files (F-029, F-025, F-017 etc) beyond
  the cross-link mentions in the F-030 PR description.
- Run the MCP rebuild or the smoke suite.
- Delete `archive/` files.

## When to stop and ask the user

- The site enumeration grew past ~7 sites and the design no longer
  collapses cleanly to a single `SetupEvalContext`.
- F-024's safe_z floor behavior conflicts with the refactor and
  preserving both adds non-trivial complexity.
- The acceptance test for F-024/F-026/F-027/F-028 fails in a way
  that doesn't yield to obvious fix.
- The user pings you with the audit session asking for status — you
  may be invoked mid-refactor by them needing a checkpoint.

## When you're done

The next acceptance loop round (run by a separate auditor session)
will verify your refactor via the smoke suite. The acceptance bar:
deflection 7/7 Within byte-identical (or within rounding noise) to
the pre-refactor round. If a smoke case regresses, that's a normal
auditor-implementer cycle handled by the next round's prompts.

Output a brief PR summary when you're done. Don't recap the design
(it's in the PR description); just: commit SHA, sites collapsed,
clippy/test status, list of dead-code helpers removed.
