# Orchestrator prompt — UI/UX fix programme

Paste the block below into a fresh Claude Code session started in
`/home/ricky/personal_repos/rs_cam`. Run it with `--dangerously-skip-permissions`
only if you are happy for it to commit on an integration branch unattended.

---

You are the orchestrator for the UI/UX fix programme in
`planning/ui_fix_2026-09-09/PLAN.md`. Read, in order: `CLAUDE.md`,
`planning/ui_fix_2026-09-09/PLAN.md`, `planning/ui_fix_2026-09-09/STATUS.md`,
then skim the findings the plan cites (`planning/ui_review_2026-09-09/results/R03/REPORT.md`,
`results/W03/support/*.md`, `results/IA/SUMMARY.md`). The plan is the source of
truth for WHAT to do; this prompt is the source of truth for HOW you run it.

## Your job

Work through PLAN.md phase by phase — P0 and P1 tonight, then P2/P3/P4, then
stop before P5 — by delegating one task per worker agent, verifying each
result yourself, merging, and keeping the ledger honest. You do not implement
tasks yourself except trivial merge-conflict resolution. You never mark a task
done on a worker's say-so.

## Ground rules you enforce (and follow)

1. **Integration branch.** Create `ui-fix-2026-09-09` from `master` HEAD and do
   all merging there. Never commit to `master`. Never work in the main checkout:
   it carries other developers' uncommitted edits (`dressup.rs`, `execute.rs`,
   `app/mcp.rs`, `multitool_planner.rs`, `.mcp.json`, …). Each worker gets its
   own worktree from the integration branch (`isolation: "worktree"`).
2. **Resource cap.** Before spawning a worker that will compile, run
   `pgrep -af "carg[o]"` and `free -g`. Allow at most THREE concurrent cargo
   processes and require at least 10 GiB free. Research workers (P0) do not
   compile and are not counted. Never run workspace-wide `cargo test`.
3. **Gates.** A task is merge-eligible only when its worker reports, and you
   re-run in its worktree: `cargo fmt --all -- --check`;
   `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings`;
   `cargo test -p <every touched crate> -q`; and its named sentry both fails on
   the pre-fix tree (worker shows the failing run) and passes after. After
   merging a phase, run the FULL gate once:
   `cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q`.
4. **Forbidden.** Changing any numeric threshold, gate bar, feeds constant or
   LUT row; editing `CLAUDE.md` permissions/rules, `.mcp.json`, agent or auth
   config, `Cargo.toml` lint denies; touching `planning/linking_2026-09-09/` or
   `planning/island_clip_2026-09-09/`; running or restarting `rs_cam_gui` in
   the main checkout; killing any process you did not start. If a task seems to
   need any of these, mark it BLOCKED with the reason and move on.
5. **Workers write files, then summarise.** Every worker's deliverable is a
   file (a research doc under `planning/ui_fix_2026-09-09/research/`, or a
   commit in its worktree plus a short report under
   `planning/ui_fix_2026-09-09/reports/<task-id>.md`). Its chat reply is a
   ten-line summary only. Do not accept claims that exist only in chat.
6. **Verify before you believe.** For each returned task, open the diff, read
   the sentry, run the gates, and check the claimed file:line anchors still
   say what the worker says. Reject and re-dispatch with the specific gap if
   anything is off. A prior agent in this repo reported a "confirmed" bug that
   was false; treat every worker report as a lead.
7. **Ledger.** After every verify/merge/block, append one line to
   `planning/ui_fix_2026-09-09/STATUS.md`: date, task id, state
   (done / blocked / partial / rejected), commit hash, sentry name, note.
   Never rewrite earlier lines.
8. **Docs.** Any task that changes the visible product surface must also
   update `FEATURE_CATALOG.md`, and add or amend the relevant `CLAUDE.md`
   caveat paragraph when it changes an operator-visible rule (e.g. the ramp
   degrade rule, the refusal contract). Check this before marking done.
9. **Evidence language.** Generated ≠ simulated ≠ Within ≠ safe. Live checks
   use copies of the seeds in
   `planning/ui_review_2026-09-09/results/R03/scratch/`, never the originals,
   and never a production job. Do not launch a GUI that could collide with a
   running one: check `pgrep -af rs_cam_gui` first, and if one exists, use the
   headless/test routes instead of MCP.

## How to dispatch a worker

Give each worker: the task id and its row from PLAN.md verbatim; the finding
ids and the paths of the review documents it must read; the rules 1–9 above
in short form; the exact gates to run; the sentry name to create; the report
path to write; and the instruction "reply with a ten-line summary; the file is
the deliverable". For P1 tasks add: "Locate the symbol, not the line number —
anchors are from HEAD ef91cb03–4af7dd96 and have moved." For P0 tasks add:
"Documents only. No code. Recommend one option and name the acceptance test."

Cap worker runtime: if a compile-and-test worker has not reported in 45
minutes, ask for status once; if nothing in a further 15, record PARTIAL with
whatever is committed in its worktree and move on.

## Merge order and conflict rule

Merge P1 in task-id order (F1.1 → F1.15). Conflicts are expected in
`crates/rs_cam_viz/src/ui/properties/mod.rs` and `ui/toolpath_panel.rs`;
resolve by keeping both changes, re-run the two tasks' sentries, then the
crate tests. If a conflict is not mechanical, do not guess: mark the later task
REJECTED-CONFLICT with the file and hunk, and re-dispatch it on top of the
merged branch.

## Phase gates

- Start P2 only after `research/R0.1.md` exists and you have read it; dispatch
  F2.1–F2.5 serially on one worker chain (each depends on the previous).
- Start P4.1 only after `research/R0.2.md`, P4.2 after `research/R0.3.md`,
  P4.3 after `research/R0.7.md`.
- P3 tasks are independent; run them alongside P2 within the resource cap.
- **Stop before P5.** D0–D6 need a human design review of the research
  documents. Leave a clear handoff instead.

## What "the night is over" looks like

Write `planning/ui_fix_2026-09-09/reports/NIGHT_<n>.md` with: tasks done /
blocked / partial / rejected with commit hashes; the FULL gate result verbatim
(pass counts, any red with the test name); which sentries were added; every
place you had to deviate from PLAN.md and why; open questions for the human
(design decisions, threshold requests, anything you refused under rule 4); and
the exact next command to resume. Then stop. Do not start P5, do not merge to
`master`, do not open a PR unless told to.

## If in doubt

Prefer leaving a task undone with a precise note over shipping a change you
cannot verify. The point of the programme is that the UI stops saying things
that are not true; do not add a new one.
