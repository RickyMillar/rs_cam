# Integration step 7.5a: rest regions, the channel and the producer (D44, D48)

Date: 2026-10-28. Branch `geodesic-integration`, base `26d901943` (7.4b).
The implementer did not commit.

Patch: `step75a_tree_vs_26d901943.patch`, md5
`3376178d53581f97a6c1bcc82c89722f`. It holds code, tests,
`FEATURE_CATALOG.md` and one folder map only (14 files). No planning file
is in it. `git apply --check -R` passes.

Split (lead: split if large): 7.5a is the XY part, D44 and D48. 7.5b is
the Z part and the refusal: D50, the D50 addendum (floor and Depth/Pass
guard) and D53, with their R tests. D55 is not ported (lead ruling).

Design §3.1 put the channel in step 1b before the producer, with planted
sources. Step 7.4 has landed the arm, so 7.5a ports the channel and the
producer together, and R's own end-to-end tests run on the product arm
with no planted source.

## 1. What changed (R commits 7bf7738a1 D44, 453d52571 D48)

| File | Change |
|---|---|
| `trace/produced.rs` | the D44 and D48 doc on `rest_regions` (tool-centre positions; a 3D Rough clips to its own containment); `has_no_rest_output` removed (its only caller changed) |
| `compute/execute.rs` | the precedence per field: the generic rest analysis runs when `rest_grid` is empty (was: both fields empty) |
| `compute/execute/dressup_apply.rs` | `attach_generic_rest_analysis` fills `rest_regions` only when the op stated none |
| `compute/execute/volumetric.rs` | `run_volumetric` drops `rest_regions` unless the op's rest analysis is on (an enabled `DerivedRestRegions` boundary names it) |
| `adaptive3d/rest_regions.rs` (new, 49 lines) | `left_regions`: the union of the left pieces, grown by one planner cell, clipped to `centre_boundary` (R `research_outer_band::left_regions` at 453d52571) |
| `adaptive3d/path.rs` | `Adaptive3dSegmentsResult::left_pieces`, taken from the context the arm already fills (7.4a) |
| `adaptive3d/mod.rs` | `mod rest_regions`; the output door sets `Produced::rest_regions` |
| `adaptive/path.rs`, `adaptive/mod.rs` | `planner_cell_size` is `pub(crate)` and re-exported, so the grow is the planner's own cell. R's typed `tool_radius / 6.0` is not ported (design §3.1 note) |
| `adaptive3d/CLAUDE.md` | `rest_regions.rs`, the D48 invariant, the three sentries; 40 lines |
| `FEATURE_CATALOG.md` | the rest analysis line: a Geodesic Adaptive rough states its own regions under the demand; the arm line points to it |
| three tests (new) | §2 |

The words "research arm" and "research clip" in the ported docs now name
Geodesic Adaptive and the outer-band clip.

## 2. The R tests, ported

From R 453d52571, with these changes only: the rough is
`ClearingStrategy::GeodesicAdaptive` / `ClearingStrategy3d::GeodesicAdaptive`
(was AgentSearch under `RS_CAM_RESEARCH_ADAPTIVE3D_ARM`, or
`Geodesic(C35_SPIRAL_PICK)`); the env assertion, the `ignore` and
`slab_field` are gone; the planner door is
`adaptive_3d_toolpath_output_with_cancel`; the step split and the route
are the arm's own (`None`). No `research` feature.

| Test | Result | Time |
|---|---|---|
| `a_rough_publishes_the_regions_it_leaves_d44` | ok: channel 1 region 249.1 mm²; stepped 1 region 249.1 mm²; pocket `None` | 15 s |
| `a_derived_rest_rough_cuts_only_the_leftovers_d44` | ok: 1 region 249.1 mm², heatmap grid attached; consumer 34 cutting moves, 0 ends outside | 11 s |
| `a_rest_consumer_stays_inside_the_source_boundary_d48` | ok | 7 s |

Defect injected: the clip off (`left_regions(.., &[])`). The D48 test is
red: "the consumer's tool crosses the rough's boundary by up to 3.447 mm
at 78 move ends". Restored.

## 3. Parity

No consumer, no publication: `adaptive3d_emission_byte_parity` 6/6 and
`every_op_keeps_its_toolpath_g_opgolden` 1/1, with no re-bless. Before
this step no op set `rest_regions` without `rest_grid`, so the new
precedence changes no shipped output.

## 4. Gates (all through the worktree `scripts/cargo_lane.sh`)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | ok |
| clippy, full features (heavy-tests, research, test-support) | ok |
| byte parity, opgolden | 6/6, 1/1 |
| the adaptive3d sentries (12 binaries), the census, `every_op_conforms_w4` | all ok |
| `geodesic_adaptive_keeps_the_research_geometry_s7_4` | 1/1 (the pins hold) |
| core `--lib` filters `adaptive3d`, `geodesic_adaptive_generates_through_the_session`, `rest` | 136 ok |
| `region_cap_honesty_f3`, `adaptive3d_subtool_channel_gouge`, `mcp_mutation_rows_reach_core`, `a_late_simulation_does_not_refill_the_core_d7` | ok |
| `folder_maps_name_real_files_f140` | 4/4 |
| `generic_rest_routing_pr7` (4 of 6 red), `crease_own_region_pr6b` (1 of 3 red) | RED ON THE BASE TOO: the same tests fail the same way on a `git archive` of 26d901943. Each op fails at generation with `NotSet { kind: Feeds, fields: [feed_rate, plunge_rate, spindle_rpm] }` (S11, unset feeds in the fixture), before any rest code runs. The same class as G-MASTERREDS-1027 (`adaptive3d_lift_bridge_b1`); these two files are not in that entry yet |

## 5. Next: 7.5b

D50 (`Produced::rest_regions_top_z`, its datum shift, `slab_top_z`, the
consumer read through `ExecutionContext` / `VolumetricInputs` /
`Adaptive3dDepth::rest_top_z`), the D50 addendum (`rest_capped_level_top`,
`rest_ceiling_keeps_doc` and their unit tests) and D53
(`refuse_larger_rest_consumer`), with R's D50 and D53 tests ported.
