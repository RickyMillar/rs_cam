# Night 1 report — UI/UX fix programme (2026-09-09 → 2026-09-10)

Orchestrator: Claude session `rs-cam-b9` (Fable 5.1), acting as the
orchestrator described in `ORCHESTRATOR_PROMPT.md`. Integration branch
`ui-fix-2026-09-09`, cut from `master` a0d21b96. Nothing was merged to
`master`; no PR was opened. The MCP server was down all night (the operator
killed it), so every MCP-VIEW acceptance is deferred and marked so.

## 1. Outcome in one table

| Item | Count | Detail |
|---|---|---|
| P0 research documents delivered | **7 of 7** | R0.1–R0.7, every one spot-checked against the code by the orchestrator |
| P1 tasks merged | **9 of 15** | F1.1, F1.2, F1.3, F1.4, F1.5, F1.6, F1.7, F1.8, F1.9 |
| P1 tasks not started | 6 | F1.10, F1.11, F1.12, F1.13, F1.14, F1.15 (see §5) |
| Tasks rejected and reworked | 1 | F1.4 (first submission regressed SVG circle drilling) |
| Sentry tests added | 9 files | one per merged task, each shown failing on the pre-fix tree |
| Commits on `ui-fix-2026-09-09` | 11 | 9 fixes + 2 docs; no merge commits, all fast-forward |
| Lines changed vs master | +11 074 / −738 across 77 files | includes the 2 811 lines of research documents |
| Merged to `master` | **none** | as instructed; no PR opened |

Integration branch `ui-fix-2026-09-09`, head **49493c13**, branched from
master **3d88406b** and containing it (no divergence).


## 2. What the night looked like

- 21:20 start. Seven P0 research agents in parallel (read-only) and one
  code worker (F1.1) in the main checkout.
- ~21:35 → 01:10 **outage 1**: the Claude session limit cut every agent.
  All seven research documents were already on disk; F1.1 was mid-clippy.
- 01:10 → 02:30 resumed; F1.1 merged; three code workers in parallel
  (main checkout + two worktrees sharing the build cache); F1.3, F1.8,
  F1.9, F1.6, F1.7 merged; F1.2, F1.4, F1.5 in flight.
- ~02:32 → 06:10 **outage 2**: second session-limit cut.
- 06:10 → morning: workers resumed time-boxed; phase gate; this report.

Roughly 7 of the ~10 available hours were lost to the two session-limit
outages. Everything below happened in the remaining ~3 hours.

## 3. Deviations from PLAN.md and ORCHESTRATOR_PROMPT.md — read these

1. **No per-task worktrees with their own build.** The disk had 21 GiB free
   and `target/debug` alone is 63 GiB (286 core test binaries). Code tasks
   ran on task branches `ui-fix/<id>` either in the main checkout or in
   source-only worktrees (`rs_cam_wt_b`, `rs_cam_wt_c`) that SHARE the main
   `target/` via `CARGO_TARGET_DIR`.
2. **The shared cache was unsafe until 01:52.** Cargo does not include the
   source path in a workspace crate's metadata hash, so two trees overwrite
   each other's `rs_cam_viz` / `rs_cam_core` artefacts and a gate in one
   tree can link the other tree's library (observed as an E0432 in the
   orchestrator's own re-run of the F1.8 sentry). Fix: `/tmp/rs_cam_gate.sh
   <tree> cargo …` — a global `flock` plus a `touch` of every `crates/**/*.rs`
   whenever the tree changes, which forces a rebuild from the current tree.
   Every task merged after that line was re-verified by the orchestrator
   THROUGH the runner. F1.1 was gated in the main checkout before any
   worktree existed (clean). F1.3's gates ran before the runner existed; its
   sentry compiled against its own module at the time (so the lib was its
   own), and it is covered by the phase gate on the merged tree.
   The runner is committed at `planning/ui_fix_2026-09-09/gate_runner.sh`
   (the `/tmp` copy does not survive a reboot).
   **Recommendation for night 2:** keep the runner, or give each worktree
   its own `CARGO_TARGET_DIR` once disk allows (~10–15 GiB each).
3. **Merges were fast-forwards by `git update-ref`** on the integration
   branch while the main checkout was occupied by a worker; task branches
   that had fallen behind were rebased in their worktree and re-gated on
   the rebased tree before the ref moved. No merge commits.
4. **Ledger timestamps** between 01:40 and 02:30 were first written from an
   estimate that ran ~1 h fast; corrected at 02:30 from the clock.
5. **Research agents wrote files** (`research/R0.x.md`) and replied with
   ten-line summaries; every document was spot-checked against the code by
   the orchestrator before it was ledgered (see STATUS.md rows for the
   exact anchors checked). Two corrections to PLAN.md came out of that
   (R0.2 leg formula; R0.5 MCP provenance stamp).

## 4. Research (P0) — all seven delivered, all spot-checked

| Doc | Lines | Headline | Spot-check by orchestrator |
|---|---|---|---|
| R0.1 freshness | 269 | Derive `FreshnessState` from the core result cache + a per-toolpath revision counter; 27 GUI flag sites replaced by one drop site. **New finding:** the inspector write-back never drops the core result, so a GUI-edited 3D op exports OLD geometry. | CONFIRMED: `properties/mod.rs::write_entry_config_to_session` writes through `find_toolpath_config_by_id_mut`; the caller sets `stale_since` + `mark_edited` only. |
| R0.2 ramp | 478 | Leg = `ENTRY_CLEARANCE`(2 mm) ÷ tan(angle) ÷ 2 = 19.08 mm at 3°, depth-independent (PLAN/review said DPP ÷ tan — corrected). Recommend option (a): fold the ramp along the following cut polyline; degrade to plunge; never refuse. | Consistent with `dressup.rs` constants/comments. Open: 10 mm frame shift between the R03 NC and the SVG. |
| R0.3 refusal | 449 | One core `preconditions.rs` predicate off `ToolConstraintsDef::allows`; op always created; eight message ids; binding rule closes F1.14. **New:** Spiral Finish is refused at add with a flat tool (feeds family Scallop) while its registry allows any tool; MCP `generate_toolpath` DOES run the static validator. **Blocking gap:** MCP cannot rebind a toolpath's tool/model. | CONFIRMED both: `REG_SPIRAL_FINISH.feeds_family = Scallop`, `validate_tool_for_operation` keys on family; `AppEvent::GenerateToolpath` pushed from `app/mcp.rs`. |
| R0.4 recheck | 440 | Two finding models exist and the GUI reads the weaker one; typed bookmark (toolpath id + kind + anchor), four events, remap rule, named subjects. | "Must address" cluster located in `sim_diagnostics.rs`. |
| R0.5 value roles | 564 | Four roles; proposed = funnel dry-run; static affected-fields table per Apply route. **Corrections:** core `set_toolpath_param` DOES stamp Manual on the five canonical fields; the pill near-match compares against the RAW recommendation (passed to F1.2). **New:** modal "Apply explored values" also rewrites plunge. | CONFIRMED: `session/compute.rs::set_toolpath_param` calls `feeds_provenance.set(.., manual())`; `value_row.rs` near-match uses `s.recommended`. |
| R0.6 planner | 449 | Apply plan deletes every `planner_origin.is_some()` op regardless of plan_id (hand-edited too); PlanChangeSummary, PLAN badge, explicit tool-draft Apply, library Save confirm, `restore_toolpath` for structural undo. | CONFIRMED: `multitool.rs::remove_planned_toolpaths` retains on `planner_origin.is_some()`. |
| R0.7 relink | 326 | "Locate file…" + relative path on save + `invalidate_model`; ~150 lines, P1-sizeable. | CONFIRMED: `load_error` has no reader under `ui/`. |

The **open questions for the human** are in each document's §7. The ones
that decide P2/P4 scope: R0.1 Q1 (stock-dimension edits stale every
toolpath?), R0.2 Q1 (fixture frame), R0.3 (Bull on Scallop: registry or
feeds wins? MCP rebind tools), R0.5 (stamp for explored-apply).

## 5. P1 tasks

| ID | State | Commit | Sentry (all fail pre-fix) | What it changes |
|---|---|---|---|---|
| F1.1 | merged | `852f710a` | `mcp_toasts_report_outcome_g_mcptoast.rs` (7) | 13 synchronous MCP arms push their toast AFTER the handler, from its reply; a refusal shows one Warning with the refusal text. Closes the "Added toolpath" toast on a refused add. |
| F1.2 | merged | `07846057` | `pill_writes_clamped_value_g_pillclamp.rs` (6) + `apply_contract_a3.rs` (16) | Every per-field suggest pill reads a dry run of the apply funnel, so it offers and writes the clamped value (1.2 mm, not 4.2) and stamps the recommendation's provenance. Near-match now compares the as-applied value. |
| F1.3 | merged | `dfa57787` | `feeds_card_labels_g_feedslabel.rs` (6) | Feeds card says "Recommended …" and prints the configured value beside it; DOC/WOC marked "(calculator)". |
| F1.4 | merged after rework | `49493c13` | `drill_no_targets_refuses_g_drillcentroid.rs` (4) | A drill op with no targets refuses instead of drilling a polygon centroid; circle-like rings (≥ 8 vertices, radii within max(2 %, 0.05 mm)) are targets, so SVG circles still drill and the star refuses. |
| F1.5 | merged | `7a668d90` | `boundary_controls_always_visible_g_boundaryinherit.rs` (3) | The dead inherit-boundary checkbox is gone; Source / Containment / Offset always visible; old project files still load. |
| F1.6 | merged | `124ca467` | `depth_beyond_stock_cautions_g_depthstock.rs` (9) | A 2.5D depth below the stock bottom raises a non-blocking caution on the header and under the depth field, in 11 forms. |
| F1.7 | merged | `58c1b55c` | `profile_through_cut_line_g_throughcut.rs` (8) | Profile names a through cut and its holding; the Tabs disclosure opens by default when no tabs are set. |
| F1.8 | merged | `b0846bcd` | `rest_badge_one_predicate_g_restbadge.rs` (9) | The Rest badge and both validators share one predecessor predicate, so the card and the generator agree. |
| F1.9 | merged | `cbe20d06` | `export_refuses_ungenerated_op_g_exportskip.rs` (6) | Export refuses an enabled operation with no result instead of silently omitting it; preflight and MCP say the same sentence. |

Two of the nine were committed BY THE ORCHESTRATOR on behalf of workers
that ran out of usage credits mid-task (F1.2, F1.5) and one more had its
rework committed the same way (F1.4). Each such commit says so in its body.
In every case the orchestrator re-ran the gates itself before committing,
and fixed one clippy failure F1.4's worker left behind (a new unit-test
module needed `expect_used` in its allow list).


Not started tonight (no worker capacity after the outages): F1.10 (drag /
reorder / GenerateAll / MCP-ladder notification), F1.11 (dirty/stale gaps —
deliberately deprioritised because R0.1 replaces the whole GUI flag scheme
in P2; adding more flag sites now would be rework), F1.12 (Ctrl+O guard +
MCP `discard_unsaved`), F1.13 (wording / Workspace menu / camera fit /
getting-started), F1.14 (superseded by R0.3's binding rule → fold into
F4.2), F1.15 (header wrap; needs MCP-VIEW).

## 6. Phase gate

Run by the orchestrator on the merged head **49493c13**, in a clean
worktree, through the locked gate runner.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` | pass, zero warnings |
| `cargo test -p rs_cam_viz -q` | **471 passed, 0 failed** |
| `cargo test -p rs_cam_core --features heavy-tests --no-fail-fast -- -q` (FULL) | **285 binaries ok, 3718 passed, 288 ignored; 2 binaries red** |

The two red binaries are `arcfit_intent_key_cost_f1` (a pinned
`(80,13)` vs measured `(74,13)` arc-fit cost) and `narrate_regions_closed_c8`
(scallop narration reports 0 regions). Neither is touched by any change in
this branch: the first is arc fitting, the second is narration, and this
programme edited feeds UI, MCP toasts, export, the drill target rule, the
boundary panel and three validator rules. Both were **already red at
master 3d88406b** — the orchestrator re-ran them there rather than
accepting a worker's word for it. THEY NEED AN OWNER, and they are not
this programme's to fix.

Per-task gates are in each `reports/F1.x.md`. Every merged task was also
re-gated by the orchestrator after rebasing, on the tree that was actually
merged, not only on the tree the worker built.

## 7. Findings surfaced by the work itself (not in the review)

- R0.1: GUI parameter edits never drop the core result → stale export of
  3D manual-regen ops. Severity S0 in the review's terms; P2 closes it.
- R0.3: Spiral Finish + flat tool refused at add; MCP has no tool/model
  rebind → a blocked MCP add is a dead end (add `set_toolpath_tool` /
  `set_toolpath_model` to P3).
- R0.5: "Apply explored values" rewrites plunge while its hover says
  "pulled down" (code-read, not live-tested).
- F1.7: `ProfileSide` has no `On` variant and the generator has no on-line
  arm, yet the schema row advertises `on` (`set_toolpath_param side=on`
  fails at serde). FEATURE_CATALOG says In Control is hidden from the
  Profile UI; the form draws it.
- F1.6: 2.5D generators ignore a pinned Bottom Z, so a Heights-tab pin
  below the stock cautions on a number that is not emitted motion.
- F1.4: usvg flattens `<circle>` to paths, so an SVG never exposes a
  `DrillTarget`; the polygon-centroid path was the ONLY way SVG circles
  drilled. Any fix must classify circle-like polygons as targets.
- F1.3: the card's advance/tooth used to print the pre-derate TARGET
  chipload; it now prints feed ÷ (RPM × flutes) at the recommended feed.
  That is a semantic change beyond a relabel — review it.
- Two core test binaries fail at the integration head and were already
  failing at master a0d21b96 (proven by the F1.4 worker by re-running at
  head): `arcfit_intent_key_cost_f1` and `narrate_regions_closed_c8`.
  Unrelated to this programme; they need an owner.

## 8. How to resume

```text
cd /home/ricky/personal_repos/rs_cam
git checkout ui-fix-2026-09-09            # integration branch
cat planning/ui_fix_2026-09-09/STATUS.md  # ledger, append-only
git worktree list                         # rs_cam_wt_b / _c / _gate may still exist — remove with `git worktree remove <path>`
```

Night 2 per PLAN §9: finish P1 (F1.10, F1.12, F1.13, F1.15; F1.11 and
F1.14 folded into P2 / F4.2), then P2 serially from R0.1 (Option A), P3 in
parallel (add `set_toolpath_tool` / `set_toolpath_model`), P4.1 from R0.2
option (a), P4.2 from R0.3, P4.3 from R0.7. **Stop before P5** — D0–D6 need
your design review of the R0.x §7 questions first.
