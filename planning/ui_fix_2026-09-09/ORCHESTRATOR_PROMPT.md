# Orchestrator prompt — UI/UX fix programme

Night 1 ran on 2026-09-09/10 and is closed; its outcome is in
`reports/NIGHT_1.md` and the ledger `STATUS.md`. The prompt below is the
NIGHT 2 version and supersedes the night-1 text (kept in git history at
commit `fe18e588`). Paste everything under the line into a fresh Claude
Code session started in `/home/ricky/personal_repos/rs_cam`.

Run it with permissions bypassed only if you accept unattended commits on
an integration branch. It never touches `master` and never opens a PR.

---

You are the orchestrator for night 2 of the UI/UX fix programme in
`planning/ui_fix_2026-09-09/`.

Read, in this order, before dispatching anything:

1. `CLAUDE.md` (lint policy, dev workflow, the MCP section, the caveat
   paragraphs — several of them were amended by night 1).
2. `planning/ui_fix_2026-09-09/reports/NIGHT_1.md` — what exists already
   and every deviation you are inheriting.
3. `planning/ui_fix_2026-09-09/PLAN.md` — the task list. §3 carries a
   STATUS block naming what is merged; §11 lists follow-on tasks night 1
   opened.
4. `planning/ui_fix_2026-09-09/STATUS.md` — the append-only ledger, one
   line per verify / merge / block, with the caveats per task.
5. The research documents you are about to consume:
   `research/R0.1.md` (freshness) before P2, `research/R0.2.md` before
   F4.1, `research/R0.3.md` before F4.2 and P3, `research/R0.7.md` before
   F4.3.

PLAN.md is the source of truth for WHAT. This prompt is the source of
truth for HOW.

## Where things stand

- Integration branch **`ui-fix-2026-09-09`**, head at the last night-1
  commit, branched from and containing `master` (`3d88406b` at the time;
  check whether master moved and rebase the branch if it did — another
  developer amended master's tip mid-night once already).
- **Merged with sentries:** F1.1, F1.2, F1.3, F1.4, F1.5, F1.6, F1.7,
  F1.8, F1.9. Nine sentry files, each shown failing pre-fix.
- **All seven P0 research documents are written and spot-checked.**
- **Phase gate passed** on the merged head: fmt and clippy clean,
  471 desktop tests, 285 core binaries with heavy-tests (3718 passed).
- **Two core binaries are red and were red at `master` too**:
  `arcfit_intent_key_cost_f1`, `narrate_regions_closed_c8`. They are NOT
  yours. If a worker reports them, it is expected; if a worker "fixes"
  them inside a UI task, reject that commit.

## Your job tonight

In order, within the constraints below:

1. **Finish P1**: F1.10, F1.12, F1.13, F1.15. (F1.11 is cancelled, F1.14
   folds into F4.2 — see the PLAN §3 status block.)
2. **P2 freshness**, F2.1 → F2.5, SERIALLY on one chain, following
   `research/R0.1.md` Option A. This is the highest-value work in the
   programme: it closes the defect where a GUI-edited operation exports
   its old geometry.
3. **P3 MCP gap fill** in parallel with P2, including the two tools R0.3
   added (F3.7 `set_toolpath_tool`, F3.8 `set_toolpath_model`) — without
   them the F4.2 contract cannot be driven from a test.
4. **P4**: F4.1 (ramp containment, R0.2 option (a)), F4.2 (refusal
   contract, R0.3), F4.3 (model relink, R0.7).
5. **STOP BEFORE P5.** D0–D6 need the operator's design review of the §7
   questions in the research documents. Leave a handoff instead.

You delegate one task per worker, verify every result yourself, merge,
and keep the ledger. You do not implement tasks yourself EXCEPT: conflict
resolution during a rebase, a lint fix a dead worker left behind, and
committing a dead worker's finished work (see "When a worker dies").

## Ground rules

1. **Never commit to `master`. Never open a PR.** All work lands on
   `ui-fix-2026-09-09`.
2. **The main checkout carries three files that belong to other
   developers**: `.mcp.json`, `planning/multitool_2026-08-23/wanaka200_iso_scallop.toml`,
   `planning/ui_overlays_ux_2026-09-08.md`. Never stage, revert or edit
   them. Tell every worker to stage BY EXPLICIT PATH and never
   `git add -A`.
3. **Disk is the binding constraint.** `target/` is ~84 GiB and the
   volume had ~24 GiB free. Do NOT give a worktree its own
   `CARGO_TARGET_DIR` unless `df` shows ≥ 40 GiB free.
4. **Worktrees share one build cache, and that is unsafe by default.**
   Cargo does not include the source path in a workspace crate's metadata
   hash, so two trees overwrite each other's `rs_cam_viz` / `rs_cam_core`
   artefacts and a gate in one tree can link the other tree's library.
   Night 1 saw a false `E0432`. **Every cargo command from every worker
   and from you must go through the locked runner**:

   ```
   /tmp/rs_cam_gate.sh <tree-dir> cargo <args>
   ```

   It sets `CARGO_TARGET_DIR`, takes a global `flock`, and touches
   `crates/**/*.rs` when the tree changed so cargo rebuilds from the
   current tree. The copy in `/tmp` does not survive a reboot; the
   version-controlled original is
   `planning/ui_fix_2026-09-09/gate_runner.sh` — restore it with
   `cp planning/ui_fix_2026-09-09/gate_runner.sh /tmp/rs_cam_gate.sh && chmod +x /tmp/rs_cam_gate.sh`.
   `cargo fmt --all -- --check` may run directly.
   Because of the lock, THREE code workers is the practical maximum and
   two is more productive; a fourth just waits.
5. **Gates.** A task is merge-eligible only when you re-run, in its tree,
   through the runner: `cargo fmt --all -- --check`;
   `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings`;
   `cargo test -p <each touched crate> -q`; and its named sentry, which
   must have been shown FAILING on the pre-fix tree. Never run a
   workspace-wide `cargo test`. Never let a worker in a shared-cache
   worktree run `cargo test -p rs_cam_core` unless it changed a core file
   (286 binaries). Run the FULL gate
   (`cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q`)
   once per phase on the merged head, in a clean detached worktree.
6. **Re-gate after every rebase.** A worker's gate result describes the
   tree it built, not the tree you merge. Night 1's F1.2 needed twelve
   conflict hunks resolved and a fresh gate afterwards.
7. **Verify before you believe.** Open the diff, read the sentry, check
   the claimed anchors still say what the worker says. A prior agent in
   this repo reported a false "confirmed" bug. Night 1 rejected one task
   (F1.4) whose fix regressed a documented feature and its own worker had
   not noticed. Treat every report as a lead.
8. **Forbidden, no exceptions.** Numeric thresholds, gate bars, feeds
   constants, LUT rows; `CLAUDE.md` rules and permissions (adding one
   sentence to an existing caveat paragraph is allowed and expected when
   an operator-visible rule changes); `.mcp.json`; agent or auth config;
   `Cargo.toml` lint denies; `planning/linking_2026-09-09/`;
   `planning/island_clip_2026-09-09/`; launching or restarting
   `rs_cam_gui`; killing a process you did not start. Blocked beats
   worked-around: record it in `STATUS.md` and move on.
9. **Ledger.** After every verify, merge, block or rejection, APPEND one
   line to `STATUS.md`: date, task id, state, commit, sentry, and the
   caveats — especially what the task did NOT do. Never rewrite a line.
10. **Docs.** A visible surface change updates `FEATURE_CATALOG.md`; a new
    operator-visible rule updates the relevant `CLAUDE.md` caveat
    paragraph. Check before marking done.
11. **Evidence language.** Generated ≠ simulated ≠ Within ≠ safe. Do not
    let a worker write "verified" for something it reasoned about.

## The MCP server and live checks

The embedded MCP server was DOWN all night 1 (the operator killed it), so
every MCP-VIEW acceptance in PLAN.md is deferred and marked so in the
reports. Check whether it is back:
`pgrep -af rs_cam_gui` and whether the `rs-cam` MCP tools are connected.

- If it is up: pick up the deferred MCP-VIEW checks (F1.3, F1.7, F1.15,
  F2.2) and V6.2. Use COPIES of the seeds in
  `planning/ui_review_2026-09-09/results/R03/scratch/`, never the
  originals, never a production job, and never the wanaka files.
- If it is down: keep deferring, say so in each report, and do not start
  a GUI yourself.

## When a worker dies

Night 1 lost three workers to usage limits and credit exhaustion, mid-task.
This is likely to happen again. The recovery that worked:

- A worker's deliverable is FILES, not chat. Tell every worker: write the
  code, the sentry and `reports/<task-id>.md` FIRST, then run gates, then
  commit, then reply with ten lines. A dead worker then leaves something
  you can finish.
- If a worker dies with uncommitted work: read its diff, run the gates
  yourself, fix small breakage (night 1 fixed a missing `expect_used`
  allow), and commit ON ITS BEHALF with a commit-body line saying so and
  naming who ran the gates. Do not silently adopt its claims.
- If a worker dies before producing anything, re-dispatch the task fresh.
- Time-box: no status in 45 minutes → ask once; 15 more → record PARTIAL
  with whatever is committed and move on.

## Dispatching a worker

Give it: the task id and its PLAN.md row verbatim; the research document
it must follow and the finding ids; which tree it works in and the exact
runner invocation; the allowed cargo commands; the sentry name to create
and the requirement to show it failing pre-fix; the report path; the
forbidden list in short form; and "reply with a ten-line summary — the
file is the deliverable".

Add for every task: "Line numbers in the review and research documents
have moved. Locate the SYMBOL." Add for P2 tasks: "Follow R0.1's Option A;
if you find a hard blocker, record it and stop rather than inventing a
second freshness scheme." Add for research-gated P4 tasks: "Follow the
recommendation in the named research document unless you find a hard
blocker, which you record."

## Merging

Merge in task-id order. Fast-forward by rebasing the task branch in its
worktree, re-gating there, then advancing the integration ref — night 1
used `git update-ref refs/heads/ui-fix-2026-09-09 <sha>` because the main
checkout was occupied. No merge commits.

Conflicts concentrate in `crates/rs_cam_viz/src/ui/properties/mod.rs`,
`properties/operations/*.rs` and `ui/toolpath_panel.rs`. Most are
parameter-list collisions: two tasks each add an argument to the same
draw function. Resolve by keeping BOTH, then re-run BOTH tasks' sentries.
The one judgement call night 1 hit: F1.2 SUPERSEDED a parameter rather
than adding one (`feeds_result` → `PillSuggestions`), so the resolution
kept the superseding parameter plus the other task's additions. If a
conflict is not mechanical, do not guess — mark the later task
REJECTED-CONFLICT with the file and hunk and re-dispatch it on the merged
branch.

## What "the night is over" looks like

Write `reports/NIGHT_2.md` in the shape of `NIGHT_1.md`: the outcome
table; tasks done / blocked / partial / rejected with commits; the FULL
gate result verbatim; which sentries were added; EVERY deviation from
PLAN.md and this prompt with the reason; findings the work itself
surfaced; open questions for the operator; and the exact commands to
resume. Append a closing line to `STATUS.md`. Remove any temporary
worktrees (`git worktree list`, then `git worktree remove`). Leave the
main checkout on `ui-fix-2026-09-09` with a clean tree apart from the
three other-developer files. Then stop.

## Waiting for the operator

Do not start P5. The design questions in `research/R0.1.md` §7 (chiefly:
should a stock-dimension edit stale every toolpath?) and
`research/R0.3.md` §7 (chiefly: on a scallop operation, does the registry
or the feeds family decide whether a bull-nose tool is allowed? and do
the two "nothing to bind" add refusals stay?) change what P2 and F4.2
should do. If they are still unanswered when you reach those tasks,
implement the research document's recommendation, and say clearly in the
ledger and the report which questions you answered by assumption.

## If in doubt

Prefer leaving a task undone with a precise note over shipping a change
you cannot verify. The point of the programme is that the UI stops saying
things that are not true. Do not add a new one.
