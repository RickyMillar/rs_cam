# W1 report: helical Waterline (D69 §2, G-WLCONTINUOUS)

Patch: `W1.patch` = `git diff a884902e6~1 a884902e6` (the lead's commit of W1, 11 files), md5
`bcb1c0a0ceb8cf19c46fbe5559e04ac7`. The first freeze (md5 d9c199f0…) held only the new test file: the path list was
one zsh variable, and zsh does not split it, so `git diff HEAD -- $P` matched nothing. The commit holds exactly the
11 paths of this report.

## What the patch does

- `WaterlineConfig::continuous` is no longer dead. Default `true` (operator ruling 2026-10-28). Help text
  `HELP_WATERLINE_CONTINUOUS`. On: the standalone Waterline op cuts each chain of whole rings over consecutive levels
  with ONE entry. Off: the stacked form, as before.
- The chain rule (`ops/waterline.rs::chain_links`) is the D69 rule, as `wl_gcode.py` prototyped it:
  1. one ring lies inside the other, to within the op's `sampling` (`Polygon2::contains_point_eps`);
  2. no third ring of the two levels lies between them (inner ⊂ w ⊂ outer, w not equal to outer);
  3. the match is the only one both ways.
  No distance bound. The topology reads every contour of the level, also a contour that the rest filter drops.
  Only a contour that is ONE whole run chains (a ring that the rest filter or the boundary splits stays stacked).
- The emit (`emit_chain`): the first ring flat at its level (one entry: rapid, `EntryPlunge`); for each next ring a
  feed at the upper Z to the nearest vertex of the lower ring, then laps of the LOWER ring that descend linearly to its
  level; last, one flat lap of the last ring; one retract. The tool descends along the lower ring, so it cannot gouge
  in 3-axis (the blocked set grows as Z falls).
- Lap count: `ceil(dz / (L · tan θ))`, θ = `compute::config::DRESSUP_RAMP_ANGLE_DEG` (3°, the repo's ramp entry
  angle; "Repo rule, no source (G10)"). The bottom of the tool cuts on the descent as on a ramp.
- The link check: each link is judged with the model gouge guard's reduced tool (cutter less `MODEL_GOUGE_TOL_MM`,
  new helper `model_gouge::reduced_tool_config`) and its Z tolerance `MODEL_GOUGE_Z_TOL_MM`, at samples no more than
  `sampling` apart. A link that fails ends the chain there: the ring gets its flat lap, the tool retracts, and the
  next ring starts a new chain. No typed number.
- Order: the level order of the stacked form. Each level emits its runs in turn; a whole ring that starts a chain emits
  the whole chain at its place. With no chain the output is the stacked form move for move (sentry 3).
- Runner: new `LayeredOp::emit_ladder` hook (`compute/execute/layered.rs`), default `None`. `run_layered` asks it
  under `RegionOrdering::ByArea` only, and only when every level cuts the whole outline. The hook gets the rest rule
  per level (`LevelRest`), so the rest filter and its tally (G-RESTLEVELS) run per level as before
  (`waterline_level_rings` is `waterline_level` without the emit).
- Spans: `compute/spans.rs::chain_barriers` adds a `RapidOrderBarrier` at the start and at the end of each cutting run
  that cuts at two or more ladder levels, so the rapid-order pass keeps each chain in its place
  (`TRANSFORM_DEPTH_ORDER` allows the in-group reorder).
- Not changed: the UnifiedFinish very-steep band and Steep and Shallow call `waterline_level` /
  `waterline_toolpath_with_cancel`; they do not read the dial.

## Saved projects

`continuous` has no serde default and is always written, so every saved project holds it. Every project saved before
this patch holds `continuous = false` (the old default; e.g. `phase3/d69/jobs/corne_play_base.toml:446`). Such a
project loads `false` and cuts the stacked form, unchanged. A new Waterline (GUI, MCP, CLI job default) gets `true`.
The old `false` carried no intent (the dial was dead). If the lead wants old projects on the helix too, that is a load
migration (false → true once); I did not add it. The GUI already has a "Continuous:" checkbox with no hover
(`rs_cam_viz/src/ui/properties/operations/surface_3d.rs:264`, I4's area; not touched).

## Pre-registration and re-bless

Pre-registration: `W1_PREREG.md` (written before the wider run).

Re-blessed (only these):

| golden | row | first differing move |
|---|---|---|
| `tests/fixtures/op_golden_g_opgolden.toml` | `waterline/fresh` | move 5 (see note) |
| `tests/fixtures/op_golden_g_opgolden.toml` | `waterline/rest` | move 5: stacked = `Rapid Retract` to (-1.675, -2.697, 13.0); helical = `FinishingCut` link to (-4.175, -6.204, 6.0) |

Note: moves 0–4 (the top ring at Z 6) are the same in both forms. The probe printed the full list for the rest arm
only; the fresh arm has the same first ring and the same count change (65 → 75 moves, 18 → 3 rapids). Both arms now
cut one chain over all 6 levels (entries 6 → 1); the rest arm's rings are all whole on this fixture.

Not re-blessed: `perf_golden_sim_metrics_3d.json` (its Waterline pins `continuous: false`, unchanged).

Tests edited (pre-registered as "may change"): two arms read the stacked form by design, so they pin
`continuous: false` with a comment:
- `a_rest_pocket_cuts_only_below_the_prior_floor_g_restlevels.rs` claim 17: it reads cut points level by level; the
  helix cuts on a descent between levels (it failed: "level 5.39…: must keep its runs").
- `link_counters_visible_g_linkvisible.rs` waterline arm: it needs many near fragments; the helix leaves 8 far ones
  (it failed: `at_depth_links 0, too_far 7`).

## New sentry

`tests/a_helical_waterline_cuts_a_stack_with_one_entry_d69.rs` (test-support; plate with a drafted outer wall and a
vertical square hole; 6 levels):
1. `a_chained_stack_descends_with_one_entry_and_no_retract`: stacked 12 entries, helical 2; each chain is one run of
   feed moves that reaches every level.
2. `no_move_of_the_helix_goes_below_the_surface`: every cutting move, sampled at 0.25 mm, stays at or above the
   drop-cutter CL Z of the guard's reduced tool (less `MODEL_GOUGE_Z_TOL_MM`), and no move goes below the lowest level.
   Red check: with the laps on the UPPER ring the test fails ("cuts 0.0202 mm below the surface").
3. `an_unmatched_ring_keeps_the_stacked_form`: one ring over two (rule 3 fails): the output equals the stacked emit,
   move for move.

## Gates (worktree, HEAD 5ca83fb6d + W1)

- `cargo fmt --all -- --check`: clean.
- `cargo clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings`: clean.
  `clippy -p rs_cam_cli -p rs_cam_mcp --all-targets -- -D warnings`: clean.
- `cargo test -p rs_cam_core -q --lib`: 2720 passed.
- Green: `a_helical_waterline_cuts_a_stack_with_one_entry_d69`, `every_op_keeps_its_toolpath_g_opgolden` (after the
  re-bless), `every_op_conforms_w4`, `every_op_states_its_capabilities_w2_4`, `a_rest_pocket_cuts_only_below_the_prior_floor_g_restlevels`,
  `link_counters_visible_g_linkvisible`, `waterline_auto_ladder_is_model_top_to_bottom_r3`,
  `waterline_respects_a_vertical_wall_r9`, `depth_beyond_stock_core_g_depthstockcore`,
  `retract_intent_move_type_census_w6`, `one_link_ceiling_for_every_op_g_linkceil`, `layered_framework_owns_the_ladder_w3`,
  `finishing_defaults_have_no_ramp_entry_r10`, `reach_tolerance_source_p5_1`, `capability_link_moves_safety`,
  `set_param_refuses_absent_field_n5`, `a_printed_cell_serves_every_family_through_one_claim_g3`,
  `the_floor_reaches_the_motion_g_bottompin`, `waterline_sees_material_above_the_flute_top_g_flutetop`,
  `waterline_shared_finish_setup_c3`, `reach_map_p5`, `retract_trip_channel_am7`, `--lib catalog`,
  `--lib compute::execute::layered`, `--lib waterline`.
- Red, NOT caused by W1:
  - `pencil_tip_float_channel_d1` (4 tests): `NotSet` feeds at generation, for Pencil as well as Waterline (S11
    fixture debt).
  - `perf_golden_sim_metrics::sim_metrics_match_golden`: only `[Profile]` fields move (air-cut time −1.1 %, arc
    WOC); W1 does not reach Profile (`emit_ladder` default `None`). HEAD moved to the C2 feed-modulation commit
    during the run; that is the likely cause. The lead should check it on HEAD alone.
- Not run: `air_cut_family_calibration_w5bf4` (heavy-tests), the viz tests (I4's area; W1 changes no viz file).

## Corne play job

Not measured: it needs a release CLI build of this worktree and a full job run (several minutes on the shared
machine). The D69 G-code estimate stands: helix in every chain, entries 282 → 94, about −589 s on the summary clock.
Note: the play project holds `continuous = false`, so a measurement needs a copy with `true`.

## Open points for the lead

1. Ramp angle: the generator cannot read the op's dressup `ramp_angle` (no dressup config in `ExecutionContext`; adding
   it touches `session/compute.rs`, I3's area). The lap uses the repo default 3°. A steeper operator angle is not
   honoured; a shallower one is not either.
2. One definition: `session/compute.rs:790` builds the guard's reduced tool inline (same 4 lines as the new
   `model_gouge::reduced_tool_config`). I3 can switch it to the helper.
3. Order and holder safety on fresh stock: a chain cuts one region down before the other regions reach that depth.
   On a rest Waterline the other regions differ only by the op's own skin (D69 §3). The chain never goes deeper than
   the stacked form.
4. Old projects stay stacked (see "Saved projects"). A one-time migration is a lead decision.
