# Handover — the UI/UX fix programme moves to the core account

Written 2026-09-11 by the outgoing orchestrator (session rs-cam-b9). The
operator has decided to finish this programme on the other account rather
than run two in parallel. That account already owns `core-consolidation`
and has finished J0–J8.

Paste everything below the line into that session.

---

You are taking over the UI/UX fix programme in `planning/ui_fix_2026-09-09/`,
in addition to the core work you already own. There is no longer a second
account; the restriction that kept you out of `crates/rs_cam_viz/**` is
LIFTED. You own all of it now.

## Read these, in this order, before doing anything

1. `CLAUDE.md` — lint policy (21 denied lints), the dev workflow table, and
   the MCP section. Several caveat paragraphs were amended by this
   programme; they are current.
2. `planning/ui_fix_2026-09-09/STATUS.md` — the append-only ledger. Every
   merge, rejection, correction and open question is one row, with what each
   task did NOT do. This is the single most useful document; read it whole.
3. `planning/ui_fix_2026-09-09/PLAN.md` — §3 for P1 status, §4 for P2 plus
   the follow-ons F2.7/F2.8/F2.10/F2.11/F2.12, §5 for P3, §6 for P4 with the
   operator's binding rulings, §7 for P5, §8 for P6, §11 for the follow-on
   list.
4. `reports/NIGHT_1.md`, then any `reports/F*.md` for a task you are about
   to touch or depend on.

## Where things stand

Integration branch **`ui-fix-2026-09-09`**, ~46 commits ahead of master,
containing master. Nothing on master. No PR. 24 sentry files added.

**Merged and verified: 31 tasks.** All seven P0 research documents; all of
P1; all of P2 (F2.1–F2.6) plus F2.9 and F2.10; four of eight P3 tools
(F3.1, F3.5, F3.7, F3.8); and two of three P4 items (F4.1 landed on your
own branch as G-RAMPCONTAIN; F4.3 landed here).

Three lane worktrees are set up and warm — `rs_cam_wt_p1`, `_p2`, `_p3` on
branches `ui-fix/lane-p1/2/3`, all clean and fully merged. Use them or
remove them; they are yours.

**Two core test binaries are red on this branch**: `arcfit_intent_key_cost_f1`
and `narrate_regions_closed_c8`. They are red at master too, and BOTH are
already fixed on your `core-consolidation`. They are not this programme's.

## The machine, and the thing you already learned the hard way

This box has **NO SWAP against 54 GiB of RAM**, so memory pressure is an
immediate OOM kill, not a slowdown. It killed two of this session's
background gates today. Your own J5 established this properly and retracted
the fabricated instruction that had been standing in for it. Cap every heavy
gate, as J5 did — `CARGO_BUILD_JOBS=2`, `--test-threads=2`, `nice`. A capped
gate is the rule; a ban was never the operator's instruction.

**Two build caches now exist and disk is no longer tight** — 188 GiB free
after 136 GiB was reclaimed from stale `target/` dirs elsewhere on the box.
Yours is `/home/ricky/personal_repos/rs_cam_target_alt`; this programme's is
the repo default at `rs_cam/target`, guarded by `/tmp/rs_cam_gate.sh`
because worktrees sharing one cache overwrite each other's artefacts (cargo
omits the source path from a workspace crate's metadata hash). If you work
in one tree with your own `CARGO_TARGET_DIR`, you need neither the lock nor
the runner. If you use the shared cache from more than one worktree, you
need both.

## How this programme verified things, and why it must continue

Every task was verified by the orchestrator before merge, not taken on
report. That caught, among other things: a fix that regressed documented
SVG circle drilling; a mechanism claim about egui that was backwards; a
worker observation that was true of its base branch and false of the merged
head; and — twice — a wrong statement that had already reached a file.

Keep these habits. They are the reason the ledger is trustworthy:

- **Re-check a claim against the MERGED head, not the branch it was written
  on.** Findings age exactly like gate results do.
- **A conflict resolution is a content decision.** "Keep both sides" is only
  safe when both sides have been read IN FULL. The outgoing orchestrator put
  a false sentence into `CLAUDE.md` by resolving a conflict from a truncated
  inspection, having already rejected that same claim in chat. It is
  recorded in the ledger at 2026-09-10 21:05.
- **A test that changes its expectation is where a fix hides its own
  weakening.** Diff every test file for REMOVED assertions before believing
  "no assertion was weakened".
- **A guard that has never failed is a guard nobody has checked.** Inject the
  violation and confirm it bites.
- **Evidence language.** Generated ≠ simulated ≠ Within ≠ safe. Nothing is
  "verified" because it was reasoned about.

## What is left, in the order to do it

### 1. Finish J0 — it is a quarter done

Your J0 landed `linking-pencil-dial` only. `isoclip-rapid`,
`ui-string-sentry` and `pipesmoke` are all still unmerged; their six commits
are on none of your branches. Verified by subject, not just ancestry. The
outgoing orchestrator described J0 as complete for most of a day and was
wrong; do not inherit that belief. Land the remaining three, gate each, and
say so in the ledger.

### 2. Merge `core-consolidation`, then rebase this branch onto it

The operator wants everything merged so a tech-debt team can start. Both red
core binaries go green when your branch lands. **Merging to master is the
OPERATOR's decision — ask, do not assume.** Once master carries it, rebase
`ui-fix-2026-09-09` onto master, re-gate, and expect the red pair to
disappear. That unblocks F4.2.

### 3. P3 — the four remaining MCP tools

`F3.2` set_face_selection, `F3.3` undo/redo, `F3.4` set_toolpath_row_control,
`F3.6` select(kind, id). PLAN §5 has the rows. Follow the shape F3.1/F3.5/
F3.7/F3.8 established: dispatch the SAME `AppEvent` the widget emits, never
reimplement the logic beside it; register in `mcp_server.rs`; mirror the
parameter structs in `rs_cam_mcp`; add schema pins; document in the CLAUDE.md
MCP section. `F3.3` pairs with F2.5, which found that undo bypassed
`invalidate_tool` in both directions — read `reports/F2.5.md` first.

### 4. P4 — F4.2, the refusal contract

The largest single remaining task. PLAN §6, driven by `research/R0.3.md`.
It needs G-BULLCUSP, which is on `core-consolidation`, so it comes after
step 2.

**Two operator rulings bind it, both recorded in PLAN §6:**

- A bull-nose tool on Scallop is **ALLOWED** (against R0.3's recommendation).
  Your own G-BULLCUSP is what makes that honest — before it, `cusp_radius`
  fell through to the envelope radius for a bull, so a widened registry would
  have spaced scallop passes off a radius 2.5× too large.
- **Spiral Finish joins the ball-tip list**, so it refuses a flat tool at the
  same surface as Scallop and Unified rather than generating and objecting
  only in the Feeds tab.

R0.3 §7 Q1, Q2 and Q5 are still unanswered; take R0.3's recommendation and
say in the ledger that you answered by assumption.

F1.14 is folded into this task (the Add menu must use the model that WILL be
bound). F3.7/F3.8 exist specifically so this contract can be driven from a
test.

### 5. P6 — validation

- **V6.1** full gate on the merged head: fmt, clippy with
  `--features rs_cam_core/heavy-tests`, and
  `cargo test -p rs_cam_core --features heavy-tests --no-fail-fast`. Capped.
- **V6.2** is the big outstanding debt. **The MCP server has been DOWN for
  this entire programme**, so every MCP-VIEW acceptance is deferred and at
  least three tasks (F1.13, F1.15, F2.2) have visual confirmation explicitly
  OWED — their sentries are source reads and say so in their own docs.
  Nothing in this programme has been seen rendered. Re-run the R03 sequence
  on COPIES of the seeds in `planning/ui_review_2026-09-09/results/R03/scratch/`,
  never the originals, never a production job, never the wanaka files. Ask
  the operator to start the GUI; do not start it yourself.
- **V6.3** sentry census: every task id maps to a named test that exists and
  passes. 24 sentry files exist; check the mapping.

### 6. P5 — do NOT start without the operator's design review

D1–D6 in PLAN §7. Comparable in size to everything done so far. The plan says
explicitly to stop before it until the operator has reviewed the §7 questions
in the research documents. Ask before starting.

### 7. The follow-on list, PLAN §11

About seventeen items the work itself opened. **Two are safety-class and the
operator has been told about both:**

- **F2.12** — the collision check has NO staleness counter, so a
  holder-clearance verdict survives any edit still reading `Clear`, and it
  feeds `readiness::holder_clearance_check`. After P2, toolpaths, export,
  simulation and undo are all honest about being out of date; the row that
  says the tool will not hit the workholding is not. Small, and squarely
  inside the model already built.
- **F4.4** — `reload_model` drops `drill_targets` and `layers`, so reloading
  a drawing whose holes MOVED keeps the previous version's targets. The
  program drills holes that are not in the file. Wrong-cut path, pre-existing.

Also there: **F1.24**, `ProjectSession::save` builds its atomic-write temp
path as `.rs_cam_save_{pid}.tmp` keyed on the pid alone, so two saves from
one process into one directory collide and one loses the rename.

## Operator decisions already made — do not re-litigate

Given in session on 2026-09-10 and recorded in the ledger at 10:10:

- **Stock edits: EVERYTHING stales** — dimensions, pins and material alone —
  on both the GUI and MCP routes. (Against R0.1's recommendation.)
- **Load regenerates 2.5D ops only**, respecting each op's own auto-regen
  dial; 3D manual-regen ops load as `NoResult`. Implemented as F2.6.
- **Bull nose allowed on Scallop.** (Against R0.3's recommendation.)
- **Spiral Finish joins the ball-tip list.**

Answered BY ASSUMPTION and still open for the operator: R0.1 §7 Q5 (the
export accept-flag is per-export and never persisted), Q6 (stale path drawn
at the existing 0.45 dim, same palette), and R0.3 §7 Q1/Q2/Q5.

## Rules

- **Never commit to `master`. Never open a PR.** Merging to master is the
  operator's decision.
- Every behaviour change ships a sentry named after the finding, SHOWN
  FAILING on the pre-fix code, with the output quoted verbatim in the report
  and commit body.
- Write the code, the sentry and the report FIRST, then gate, then commit.
- Stage BY EXPLICIT PATH. Never `git add -A` — `.mcp.json` in this checkout
  carries another developer's uncommitted change and must not be staged.
- **Write reports and ledger rows into the worktree you are working in.**
  Four of your J reports and two ledger rows were written into the main
  checkout by mistake and had to be rescued; check your working directory.
- Forbidden: numeric thresholds, gate bars, feeds constants, LUT rows;
  `CLAUDE.md` rules and permissions (adding a sentence to an existing caveat
  paragraph is allowed and expected when an operator-visible rule changes);
  `.mcp.json`; agent or auth config; `Cargo.toml` lint denies; launching or
  restarting `rs_cam_gui`; killing a process you did not start.
- Append one line per task to `STATUS.md`. It is append-only; never rewrite
  a line. Record what the task did NOT do — that is what made this ledger
  worth reading.

## What "finished" looks like

Write `reports/NIGHT_3.md` in the shape of `NIGHT_1.md`: outcome table;
tasks done / blocked / partial / rejected with commits; the FULL gate result
verbatim; sentries added; EVERY deviation from PLAN.md and this prompt with
the reason; findings the work itself surfaced; open questions for the
operator; and the exact commands to resume. Append a closing row to
`STATUS.md`. Remove the temporary worktrees you no longer need. Leave the
main checkout on `ui-fix-2026-09-09` with a clean tree apart from
`.mcp.json`.

The point of this programme is that the UI stops saying things that are not
true. Do not add a new one — including in a report, a commit message, or
`CLAUDE.md`. Prefer leaving a task undone with a precise note over shipping
a change you cannot verify.
