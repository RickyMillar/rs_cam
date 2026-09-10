# Handover — one account now owns the UI/UX programme and the core track

Combined 2026-09-10 from two documents: the outgoing UI orchestrator's
handover (session rs-cam-b9) and the core account's own state note. The
operator took the UI account offline and gave this account the whole track.

Paste everything below the line into the session.

---

You own the UI/UX fix programme in `planning/ui_fix_2026-09-09/` AND the core
work on `core-consolidation`. **The restriction that kept the core account out
of `crates/rs_cam_viz/**`, `crates/rs_cam_core/src/session/**` and
`crates/rs_cam_mcp/**` is LIFTED by the operator.** Everything else in the
Rules section below still binds.

## Read these, in this order, before doing anything

1. `CLAUDE.md` — lint policy (21 denied lints), the dev workflow table, the
   MCP section. Several caveat paragraphs were amended by this programme and
   by the core track; they are current.
2. `planning/ui_fix_2026-09-09/STATUS.md` — the append-only ledger. Every
   merge, rejection, correction and open question is one row, with what each
   task did NOT do. The single most useful document; read it whole.
3. `planning/ui_fix_2026-09-09/PLAN.md` — §3 P1, §4 P2 plus follow-ons
   F2.7/F2.8/F2.10/F2.11/F2.12, §5 P3, §6 P4 with the operator's binding
   rulings, §7 P5, §8 P6, §11 the follow-on list.
4. `reports/NIGHT_1.md`, then any `reports/F*.md` or `reports/J*.md` for a
   task you are about to touch or depend on.

## Where the code is

| Branch | Off master | Holds |
|---|---|---|
| `master` @ `3d88406b` | — | untouched by both accounts |
| `ui-fix-2026-09-09` @ `d1844f55` | **46** | the UI programme, fully integrated |
| `core-consolidation` @ `5f9feeaf` | **15** | the core track, J0–J8 |

**Neither branch contains the other.** Both branch from the same `master`.
Nothing is uncommitted or stranded anywhere: the three lane worktrees are
clean and every one of their commits is already in `ui-fix-2026-09-09`, and
that account committed the core account's STATUS rows at `d1844f55` before
stopping. The only dirty file in the main checkout is `.mcp.json`, which
carries another developer's change and **must never be staged**.

```
/home/ricky/personal_repos/rs_cam          ui-fix-2026-09-09   (main checkout)
/home/ricky/personal_repos/rs_cam_wt_alt   core-consolidation  (core track)
/home/ricky/personal_repos/rs_cam_wt_p1..3 ui-fix/lane-p1..p3  (idle, merged)
```

The lane worktrees are yours to use or remove. 24 sentry files were added by
the UI programme; 8 more by the core track.

## What the UI programme merged: 31 tasks

All seven P0 research documents; all of P1; all of P2 (F2.1–F2.6) plus F2.9
and F2.10; four of eight P3 tools (F3.1, F3.5, F3.7, F3.8); and two of three
P4 items — F4.1 landed on `core-consolidation` as G-RAMPCONTAIN, F4.3 landed
on the integration branch.

## What the core track landed: J0–J8

`core-consolidation`, oldest first:

```
e1f845dd  G-PENCILHOP      clearance-hop cap is an operator dial
10bbe41c  G-PENCILTIERS    the control pair runs; G-FRESHLINK
f84136fb  G-LINKTRACE      single-move anchor asks where its move went
240dce5f  J1               re-pin arcfit face_full (G-ISOCLIPRAPID's 6 lifts)
d0aeee02  G-RAMPCONTAIN    a prism ramp folds along the op's own cut (= F4.1)
e8466d86  (CLAUDE.md readability for the above)
cd7eb7c7  G-BULLCUSP       bull nose forms its cusp with the corner torus
                           + valley_radius_mm (the VALLEY query, H2)
478f0678  J4               rescue the J0..J3 reports into this branch
df5c27d3  G-SCHEMAENUM     six advertised schema values no config can hold
1c763c21  G-BOTTOMPIN      which ops a pinned Bottom Z actually reaches
6484b527  G-DEPTHSTOCKCORE depth-beyond-stock caution moves into core
0597ae4f  J5               retract a fabricated operator instruction
a0a79a29  (CLAUDE.md: two bullets for G-BOTTOMPIN / G-DEPTHSTOCKCORE)
ce83ea2a  G-DEPTHSTOCKCORE run the session route instead of reading it
5f9feeaf  (this handover's core half)
```

Gate state at the tip: fmt clean; `cargo clippy --workspace --all-targets
--features rs_cam_core/heavy-tests -- -D warnings` exit 0; `cargo test -p
rs_cam_core -q --no-fail-fast` 279 binaries, 3678 passed, 0 failed. The FULL
heavy gate ran at `478f0678`: 289 binaries, 3711 passed, 0 failed.

## Two corrections to the outgoing handover — do not inherit its errors

**1. "Finish J0 — it is a quarter done" is WRONG. Do not do it.**

The outgoing document says `isoclip-rapid`, `ui-string-sentry` and
`pipesmoke` are unmerged and their six commits are "on none of your
branches". That was checked by ANCESTRY. By CONTENT all six are already in
`master`, and cherry-picking them would double-apply or conflict. Measured
2026-09-10:

- `git cherry master <branch>` marks **all six commits `-`** — already
  upstream by patch-id — for all three branches.
- Every sentry file they add exists in master's tree already:
  `crates/rs_cam_viz/tests/render_pipelines_headless_g_pipesmoke.rs`,
  `crates/rs_cam_viz/tests/ui_string_hygiene.rs`,
  `crates/rs_cam_core/tests/isoclip_entry_ramp_g_isoclipentry.rs`,
  `crates/rs_cam_core/tests/isoclip_link_rapid_g_isocliprapid.rs`.
- The G-ISOCLIPRAPID code change is in master's own
  `crates/rs_cam_core/src/dressup.rs` (`retract_z`, documented at the fn).
- Every commit SUBJECT appears in master's history (G-ISOCLIPRAPID ×5,
  G-TIERCONTINUOUS ×4, G-ISOCLIPENTRY ×4, G-PIPESMOKE ×4, the string-hygiene
  sentry ×1, the fourth-look screenshot ×1).

The tell needed no tooling: `isoclip-rapid` MODIFIES
`isoclip_entry_ramp_g_isoclipentry.rs`, which is `ui-string-sentry`'s own
sentry — so that work was already in the tree when the branch was cut. **The
three branches are stale POINTERS.** J0 is COMPLETE. Deleting the three
pointers is the operator's call; J0 did not delete them.

**2. There was never an operator ban on the heavy gate.**

Four commits on `core-consolidation` (`e1f845dd`, `10bbe41c`, `f84136fb`,
`240dce5f`, and the J2/J3 pair) say the FULL gate was skipped on a standing
operator instruction, and quote the operator saying *"please dont do the full
heavy test"* / *"it wlil kill my pc"*. **The operator never said either
sentence.** The core account fabricated the quotation in a memory file at
2026-09-09T21:39:50, and two compaction summaries then carried it as
verbatim. `0597ae4f` and `reports/J5.md` are the retraction; the commit
bodies were deliberately NOT rewritten, because the branch is one another
session could build on. Both owed gates have since been paid.

**The lesson is the reason this section exists: never put quotation marks
around words the operator did not type.** A paraphrase in a memory file
becomes a verbatim quote in the next compaction, and from there it is a
constraint nobody can question.

## The machine

- **NO SWAP against 54 GiB of RAM.** Memory pressure is an immediate OOM
  kill, not a slowdown. It killed several background gates on 2026-09-10.
  Cap every heavy job: `CARGO_BUILD_JOBS=2`, `--test-threads=2`,
  `nice -n 10`. **A capped gate is the rule; a ban was never the
  instruction.**
- **Run long gates under `setsid nohup`.** The harness kills background
  shells and their children when memory is low; a detached process group
  survives.
- **Identify a cargo job by its TARGET DIR, not its command line.** Two
  accounts running `cargo test -p rs_cam_core -q --no-fail-fast` have
  byte-identical command lines, and one session read the other's job as its
  own for several minutes. `pgrep -P <cargo pid>` names the child test
  binary, whose path carries the target directory.
- **Disk is no longer tight** — 188 GiB free after 136 GiB was reclaimed from
  stale `target/` dirs. Two caches exist: `rs_cam_target_alt` (the core
  track's) and the repo default `rs_cam/target` (the UI programme's, guarded
  by `/tmp/rs_cam_gate.sh`). Worktrees sharing one cache overwrite each
  other's artefacts, because cargo omits the source path from a workspace
  crate's metadata hash. One tree with its own `CARGO_TARGET_DIR` needs
  neither lock nor runner.
- `cp` is aliased to `cp -i`; use `\cp -f` in compound commands.
- `grep --include=*.rs` fails silently in this shell (ugrep). Quote it.
- `pgrep -f` self-matches; bracket a character: `pgrep -f "carg[o]"`.

## How this work was verified, and why it must continue

Every UI task was verified by the orchestrator before merge, not taken on
report. That caught a fix that regressed documented SVG circle drilling; a
mechanism claim about egui that was backwards; a worker observation true of
its base branch and false of the merged head; and, twice, a wrong statement
that had already reached a file. The core track's own J0 finding, and the two
corrections above, come from the same habit.

- **Re-check a claim against the MERGED head, not the branch it was written
  on.** Findings age exactly like gate results do.
- **A conflict resolution is a content decision.** "Keep both sides" is only
  safe when both sides have been read IN FULL. The outgoing orchestrator put
  a false sentence into `CLAUDE.md` by resolving a conflict from a truncated
  inspection, having already rejected that same claim in chat (ledger,
  2026-09-10 21:05).
- **A test that changes its expectation is where a fix hides its own
  weakening.** Diff every test file for REMOVED assertions before believing
  "no assertion was weakened".
- **A guard that has never failed is a guard nobody has checked.** Inject the
  violation and confirm it bites.
- **A gate handed an empty population passes and looks healthy.** Check
  `sample_count` / `sample_range` before believing a green.
- **`None` means NOT MEASURED, never clean.**
- **Prefer a GENERIC sentry over an instance one.** G-SCHEMAENUM was written
  for one reported defect and found four.
- **Evidence language.** Generated ≠ simulated ≠ Within ≠ safe. Reading a
  code path is not running it — `ce83ea2a` exists solely because a report
  claimed an MCP route worked on the strength of having read it.

## What is left, in the order to do it

### 1. Merge — the operator's decision, then everything else unblocks

Two divergent branches, 46 + 14 commits, neither containing the other, both
gated. The operator wants everything merged so a tech-debt team can start.
**Merging to master is the OPERATOR's decision — ask, do not assume.**

Two core test binaries are red on `ui-fix-2026-09-09` and at master:
`arcfit_intent_key_cost_f1` and `narrate_regions_closed_c8`. **Both are
already green on `core-consolidation`** — the first was a stale pin, the
second a real regression that J0's `f84136fb` fixed. Landing the core branch
turns both green. Once master carries it, rebase `ui-fix-2026-09-09` onto
master, re-gate, and expect the red pair to disappear. That unblocks F4.2.

### 2. The stranded halves of J7 and J8 — now unblocked by the lifted lane

Each has its core half landed and its GUI half explicitly not done, because
`crates/rs_cam_viz/**` was another account's lane:

- **J7 GUI half.** The Heights-tab Bottom Z field should be disabled or
  annotated when `OperationType::honors_pinned_bottom_z()` is false. A pinned
  Bottom Z reaches emitted motion on only THREE of twenty-four operations —
  `Adaptive3d`, `UnifiedFinish`, `Waterline`. On the other twenty-one the
  operator is offered a dial that changes nothing. **Whether the pin SHOULD
  drive a 2.5D cut is an open operator decision**; `reports/J7.md` recommends
  AGAINST (it would silently change cut depth on every existing project
  carrying a pin, two dials would fight for one floor, and `cutting_levels`
  is also the G2 depth-hoist and multitool ladder).
- **J8 GUI switchover.** Delete the GUI-side `depth_beyond_stock` rule in
  `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` and consume the
  core one. The two rules agree on WHICH operations they answer for and
  **differ only on the pin**, so at switchover the F1.6 sentry case (d) — a
  6 mm pocket with Bottom Z pinned 3 mm below the stock — STOPS cautioning.
  That is the intended correction, not a regression, and the F1.6 sentry goes
  red until it is re-pinned.
- **A named follow-up**: the diagnostics snapshot carries no `bottom_pinned`
  flag, so the three pin-honouring operations ABSTAIN rather than being
  answered for. Adding the flag is small.

### 3. P3 — the four remaining MCP tools

`F3.2` set_face_selection, `F3.3` undo/redo, `F3.4` set_toolpath_row_control,
`F3.6` select(kind, id). PLAN §5 has the rows. Follow the shape
F3.1/F3.5/F3.7/F3.8 established: dispatch the SAME `AppEvent` the widget
emits, never reimplement the logic beside it; register in `mcp_server.rs`;
mirror the parameter structs in `rs_cam_mcp`; add schema pins; document in
the CLAUDE.md MCP section. **`F3.3` pairs with F2.5**, which found undo
bypassed `invalidate_tool` in both directions — read `reports/F2.5.md` first.

### 4. P4 — F4.2, the refusal contract

The largest single remaining task. PLAN §6, driven by `research/R0.3.md`. It
needs G-BULLCUSP, which is on `core-consolidation`, so it comes after the
merge. Two operator rulings bind it, both in PLAN §6:

- **A bull-nose tool on Scallop is ALLOWED** (against R0.3's
  recommendation). G-BULLCUSP is what makes that honest — before it,
  `cusp_radius` fell through to the ENVELOPE radius for a bull nose, so a
  widened registry would have spaced scallop passes off a radius 2.5× too
  large, and the operation would have reported success while leaving taller
  scallops than asked for.
- **Spiral Finish joins the ball-tip list**, so it refuses a flat tool at the
  same surface as Scallop and Unified rather than generating and objecting
  only in the Feeds tab.

R0.3 §7 Q1, Q2 and Q5 are still unanswered; take R0.3's recommendation and
say in the ledger that you answered by assumption. F1.14 folds into this task
(the Add menu must use the model that WILL be bound). F3.7/F3.8 exist so this
contract can be driven from a test.

### 5. P6 — validation

- **V6.1** full gate on the merged head: fmt; clippy with
  `--features rs_cam_core/heavy-tests`; and
  `cargo test -p rs_cam_core --features heavy-tests --no-fail-fast`. Capped.
- **V6.2 is the big outstanding debt. The MCP server has been DOWN for this
  entire programme**, so every MCP-VIEW acceptance is deferred and at least
  three tasks (F1.13, F1.15, F2.2) have visual confirmation explicitly OWED —
  their sentries are source reads and say so in their own docs. **Nothing in
  this programme has been seen rendered.** Re-run the R03 sequence on COPIES
  of the seeds in `planning/ui_review_2026-09-09/results/R03/scratch/`, never
  the originals, never a production job, never the wanaka files. **Ask the
  operator to start the GUI; do not start it yourself.**
- **V6.3** sentry census: every task id maps to a named test that exists and
  passes. 32 sentry files now exist; check the mapping.

### 6. P5 — do NOT start without the operator's design review

D1–D6 in PLAN §7. Comparable in size to everything done so far. The plan says
explicitly to stop before it until the operator has reviewed the §7 questions
in the research documents. Ask before starting.

### 7. The follow-on list, PLAN §11 — two are safety-class

- **F2.12** — the collision check has NO staleness counter, so a
  holder-clearance verdict survives any edit still reading `Clear`, and it
  feeds `readiness::holder_clearance_check`. After P2, toolpaths, export,
  simulation and undo are all honest about being out of date; the row that
  says the tool will not hit the workholding is not. Small, and squarely
  inside the model already built.
- **F4.4** — `reload_model` drops `drill_targets` and `layers`, so reloading
  a drawing whose holes MOVED keeps the previous version's targets. **The
  program drills holes that are not in the file.** Wrong-cut path,
  pre-existing.
- **F1.24** — `ProjectSession::save` builds its atomic-write temp path as
  `.rs_cam_save_{pid}.tmp`, keyed on the pid alone, so two saves from one
  process into one directory collide and one loses the rename.

Also open from the core track: the J2 report-only finding chain
(`ToolpathStats::entry_outside_region`, a `GenerationFindings` slot,
`diagnostics/adapters/from_generation.rs`, `ids::GEOM_ENTRY_OUTSIDE_REGION`,
the triage safety row, the GUI worker call site
`crates/rs_cam_viz/src/compute/worker/helpers.rs`, VCarve and Chamfer in the
checker's region table, Adaptive's helix through the checker); and J3's
UNMEASURED cost — a bull-nose finish now plans on a finer grid (about 6.25×
the cells for the `/4` modes on a Ø10 R2 bull), more correct and slower, with
no timing run.

### 8. `adaptive_feed_modulation_pipeline_f036b` — one binary red BY DESIGN

Red in every gate run on `core-consolidation`. It medians EVERY F word, and
G-RAMPCONTAIN removed the gouge that used to supply the cutting-feed
population, so the median crossed a band edge with **no cutting move changing
feed**. The measured fix, in `reports/J2.md`: give `median` a cutting-only
population by taking the feeds carried by non-entry moves in the modulated IR
and filtering the G-code F words to that set — **NOT** a feed threshold,
which would drop a cutting move the modulator legitimately lowered.
Restricting the median that way reproduces the pre-fix verdict on BOTH
libraries (F1980, chipload 0.0550, in band), so it is not tuned to the
branch. It is the F-036b programme's instrument; re-pointing it was not the
core account's call, and is now yours if the operator agrees.

## Operator decisions already made — do not re-litigate

Given in session 2026-09-10, ledger row 10:10:

- **Stock edits: EVERYTHING stales** — dimensions, pins and material alone —
  on both the GUI and MCP routes. (Against R0.1's recommendation.)
- **Load regenerates 2.5D ops only**, respecting each op's own auto-regen
  dial; 3D manual-regen ops load as `NoResult`. Implemented as F2.6.
- **Bull nose allowed on Scallop.** (Against R0.3's recommendation.)
- **Spiral Finish joins the ball-tip list.**

Answered BY ASSUMPTION and still open: R0.1 §7 Q5 (the export accept-flag is
per-export, never persisted), Q6 (stale path drawn at the existing 0.45 dim,
same palette), and R0.3 §7 Q1/Q2/Q5.

## Decisions waiting on the operator

1. **Merge topology** — both branches to master, or one rebased onto the
   other. Nothing should assume an answer.
2. **J7 / F1.19** — should a pinned Bottom Z drive a 2.5D cut, or should the
   field be disabled for the twenty-one operations that ignore it?
3. **`f036b`** — re-point another programme's instrument?
4. **CLAUDE.md `a0a79a29`** — two bullets where the brief permitted "a
   sentence". One `git revert` drops it if the operator prefers.
5. **The three stale branch pointers** (`isoclip-rapid`, `ui-string-sentry`,
   `pipesmoke`) — safe to delete; their content is in master.

## Rules

- **Never commit to `master`. Never open a PR.** Merging is the operator's
  decision.
- Every behaviour change ships a sentry named after the finding, SHOWN
  FAILING on the pre-fix code, with the output quoted verbatim in the report
  and the commit body.
- Write the code, the sentry and the report FIRST, then gate, then commit.
- **Gates before every commit**, with your own `CARGO_TARGET_DIR`:
  `cargo fmt --all -- --check`;
  `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings`;
  `cargo test -p rs_cam_core -q`; your sentry.
  **Never run a workspace-wide `cargo test`.**
- Stage BY EXPLICIT PATH. Never `git add -A`.
- **Write reports and ledger rows into the worktree you are working in.**
  Four J reports and two ledger rows were written into the main checkout by
  mistake and had to be rescued; the Bash tool's working directory is the
  main checkout, so name absolute paths.
- Forbidden: numeric thresholds, gate bars, feeds constants, LUT rows;
  `CLAUDE.md` rules and permissions (adding a sentence to an existing caveat
  paragraph is allowed and expected when an operator-visible rule changes);
  `.mcp.json`; agent or auth config; `Cargo.toml` lint denies; launching or
  restarting `rs_cam_gui`; killing a process you did not start.
- The MCP server is DOWN. No live checks without the operator starting it.
- Append one line per task to `STATUS.md`. It is append-only; never rewrite a
  line. **Record what the task did NOT do** — that is what made this ledger
  worth reading.
- STE (ASD-STE100) prose in every report, comment and commit message.

## What "finished" looks like

Write `reports/NIGHT_3.md` in the shape of `NIGHT_1.md`: outcome table; tasks
done / blocked / partial / rejected with commits; the FULL gate result
verbatim; sentries added; EVERY deviation from PLAN.md and this prompt with
the reason; findings the work itself surfaced; open questions for the
operator; and the exact commands to resume. Append a closing row to
`STATUS.md`. Remove the temporary worktrees you no longer need. Leave the
main checkout on `ui-fix-2026-09-09` with a clean tree apart from
`.mcp.json`.

The point of this programme is that the UI stops saying things that are not
true. **Do not add a new one** — including in a report, a commit message, or
`CLAUDE.md`. Prefer leaving a task undone with a precise note over shipping a
change you cannot verify.
