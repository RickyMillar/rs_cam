# Integration step 7.3d: the research module becomes test support

Date: 2026-10-28. Branch `geodesic-integration`, base `d639bb8f1` (7.3b).
The implementer did not commit.

Patch: `step73d_tree_vs_d639bb8f1.patch`, md5
`a1fffb6d146f45201c386fa0ad3c414a` (8 files: 286 insertions, 8553
deletions). `git apply --check -R` passes.

## 1. What changed

| File | Change |
|---|---|
| `adaptive/research_geodesic.rs` | 9 966 → 2 128 lines. It plans nothing: the readouts, the cascade, the turn feeds, the entry split, the reports and the harness door `run_2d`/`run_2d_with`. `Candidate2d::Geodesic` has no payload and plans the product preset through `adaptive::geodesic` |
| `adaptive/geodesic/plan.rs` | `#[cfg(feature = "research")]` test-support door `research_plan_piece` and `ResearchPlan` (the raw plan of one piece: segments, turns, wraps, spiral, seed, `K`, level count); `Produced` carries it under the same `cfg`. `PieceSetup`, `planner_cell`, `params_with` are `pub(crate)` for the readouts |
| `adaptive/geodesic/mod.rs` | the `research` re-exports (`params_with` as `research_params_with`, because the door imports the same name) and `#[cfg(test)] mod tests` |
| `adaptive/geodesic/tests.rs` (new, 563 lines) | the seven unit tests whose subject is product code (§3), with test-only helpers (`lattice_over`, `over_part`, `distance_mm`, `branch_count`, `level_values_mm`, `law_next_radius`, `level_is_star_shaped`) |
| `tests/adaptive_geodesic_2d_renders_g1.rs` | 7 columns in place of 25+: `agent`, `contour_spiral`, `geodesic`, `geodesic_fine`, and the three area columns. The report line holds the reduced field report; `c16_text` and `holding_text` are gone |
| `tests/the_geodesic_preset_keeps_its_plan_s7_3.rs` | `Candidate2d::Geodesic` (no options); the doc names the deletion |
| `adaptive/CLAUDE.md`, `adaptive/geodesic/CLAUDE.md` | 40 lines each; the research file is test support; the `tests.rs` sentry |

The research `FieldReport` keeps `applied`, `seed`, `seed_radius`,
`levels`, `chains`, `spiral_levels`, `turns`. `StepPart` lost `Morph`,
`Return`, `Transition` and `LeadIn`; `EntrySplit` lost `clearing_lead`
(no plan makes them).

## 2. Deleted (R = research branch 607e52730; the file is the same at 453d52571)

78 top-level items of R `adaptive/research_geodesic.rs`. Spans run to the
line before the next item. 45 other items live on in `adaptive/geodesic/`,
46 in the research file, and 27 (the 3D slice layer, R :8189–9091) wait
for step 7.4 (`slice.rs`).

| R lines | Item |
|---|---|
| 401-421 | `FieldSolver` (enum) |
| 1609-1659 | `join_fragments` (fn) |
| 1929-1945 | `MeetingRule` (enum) |
| 1946-1997 | `JoinRule` (enum) |
| 1998-2052 | `LobeMorph` (enum) |
| 2053-2074 | `Spacing` (enum) |
| 2075-2088 | `Chaining` (enum) |
| 2089-2126 | `Reseed` (enum) |
| 2127-2148 | `ArcLink` (enum) |
| 2149-2169 | `ArcEnds` (enum) |
| 2170-2195 | `ArcPick` (enum) |
| 2196-2212 | `CoreJoin` (enum) |
| 2213-2231 | `GeodesicOptions` (struct) |
| 2232-2428 | `GeodesicOptions` (impl) |
| 2429-2438 | `ParentRef` (enum) |
| 2511-2541 | `nearest_on` (fn) |
| 2653-2662 | `climb_oriented` (fn) |
| 2663-2711 | `right_growth` (fn) |
| 2712-2740 | `arc_end_lens` (fn) |
| 2741-2791 | `wall_normal` (fn) |
| 2792-2844 | `end_lens_at` (fn) |
| 2845-2862 | `ordered_wraps` (fn) |
| 3115-3123 | `EntryLaw` (struct) |
| 3124-3250 | `EntryLaw` (impl) |
| 3251-3268 | `circle_from` (fn) |
| 3294-3326 | `circular_front_width` (fn) |
| 3327-3349 | `transition_fraction` (fn) |
| 3350-3370 | `TransitionRecord` (struct) |
| 3371-3398 | `JoinReport` (struct) |
| 3399-3487 | `transition_wrap` (fn) |
| 3488-3538 | `curvature_levels` (fn) |
| 3539-3567 | `core_spiral` (fn) |
| 3718-3724 | `lobe_length_ratio` (fn) |
| 3725-3739 | `LobeStop` (enum) |
| 3740-3785 | `LobeReport` (struct) |
| 3786-3821 | `resample` (fn) |
| 3822-3827 | `band_samples` (fn) |
| 3828-3844 | `aligned_to` (fn) |
| 3845-3870 | `lobe_link` (fn) |
| 3871-3886 | `PieceKind` (enum) |
| 3887-3893 | `PlanPiece` (struct) |
| 3894-3907 | `blend` (fn) |
| 3908-4016 | `morph_legs` (fn) |
| 4017-4027 | `lead_in_radius` (fn) |
| 4028-4036 | `LeadContext` (struct) |
| 4037-4055 | `all_inside` (fn) |
| 4056-4067 | `lead_in_radius_pivot` (fn) |
| 4068-4100 | `lead_in_onto` (fn) |
| 4101-4189 | `lead_band` (fn) |
| 4190-4233 | `lead_legs` (fn) |
| 4234-4304 | `lead_closed` (fn) |
| 4305-4567 | `morph_plan` (fn) |
| 4619-4665 | `ArcReport` (struct) |
| 4666-4693 | `JoinClass` (enum) |
| 4694-4701 | `JoinMark` (struct) |
| 4702-4710 | `PickKept` (enum) |
| 4711-4723 | `PickRecord` (struct) |
| 4724-4752 | `pick_score` (fn) |
| 4753-4771 | `pick_kept` (fn) |
| 4772-4781 | `OverBoundTurn` (struct) |
| 4782-4830 | `OverBoundTurn` (impl) |
| 4831-4871 | `SplitReport` (struct) |
| 4872-4894 | `ReseedField` (struct) |
| 4895-4910 | `ReseedStop` (enum) |
| 4911-4921 | `ReseedReport` (struct) |
| 4922-4931 | `ReseedReport` (impl) |
| 5002-5021 | `clearance` (fn) |
| 5062-5127 | `geodesic_segments` (fn) |
| 5728-5738 | `ReplayModel` (struct) |
| 5739-5949 | `ReplayModel` (impl) |
| 5950-6046 | `cleared_nodes` (fn) |
| 6047-6188 | `reseed_segments` (fn) |
| 6189-6238 | `seam_joins` (fn) |
| 6239-6278 | `far_side_zone` (fn) |
| 6279-6320 | `zone_labels` (fn) |
| 6321-6335 | `branches_in` (fn) |
| 6336-6400 | `meeting_cells_in` (fn) |
| 6401-6551 | `split_segments` (fn) |

Methods deleted from kept `impl` blocks: `Lattice::over` (:360), `GeodesicField::reseeded` (:853), `GeodesicField::grown_from` (:862), `GeodesicField::with_inside` (:952), `GeodesicField::pivot_near` (:982), `GeodesicField::use_local_spacing` (:1002), `GeodesicField::use_curvature_spacing` (:1058), `GeodesicField::converged` (:1325), `GeodesicField::branches_of` (:1589), `GeodesicOptions::pick_variants` (:2413), `EntryLaw::tilt` (:3143), `EntryLaw::advance` (:3151), `EntryLaw::ramp` (:3161), `EntryLaw::spiral` (:3202), `OverBoundTurn::fits_slanted` (:4824), `ReseedReport::reseeds` (:4926), `ReplayModel::material` (:5756), `ReplayModel::cut_start` (:5784), `ReplayModel::walk` (:5827), `ReplayModel::plan_wrap` (:5884).

So step 7.7 has left only the move of the C12 judge onto the product arm
(and the arm's own option, if any): `GeodesicOptions` and every preset are
already gone.

## 3. The research unit tests

| Test | Fate |
|---|---|
| `the_field_on_a_disc_is_the_euclidean_distance`, `the_field_goes_round_an_island`, `the_five_arm_axes_of_the_star_read_alike` | moved to `geodesic/tests.rs`; the first-order solver rows and the `converged` asserts went with the first-order solver (`on_lattice` keeps no `converged`) |
| `the_whole_chaining_keeps_the_segment_the_tail_chaining_drops`, `the_local_level_spacing_follows_the_law_on_a_disc`, `the_star_shape_test_rejects_a_level_set_behind_an_island`, `only_the_core_loop_leaves_the_wraps_beside_a_core_spiral` | moved to `geodesic/tests.rs`, unchanged in substance |
| `a_straight_side_cut_reads_alike_in_both_measures`, `a_head_on_step_up_a_wall_is_over_the_ceiling_in_both_measures`, `two_slivers_read_wide_in_swept_width_and_narrow_in_area`, `a_turn_whose_vertex_the_emitter_drops_is_unfed_only_when_it_needs_a_cut_feed`, `the_turn_feed_holds_the_steady_removal_and_the_lateral_force` | stay (readout subjects) |
| `every_emitted_step_holds_the_pass_load` (ignored, G-MOPLOAD), `every_wrap_lies_inside_the_tool_centre_region` | stay; run the product preset in place of `C2` |
| `a_clearing_turn_is_bounded_and_fed_down_and_no_other_step_passes` | stays; runs the product preset (zigzag) in place of the C14 pair, and reads the turn count and the turn records in place of the deleted `ArcReport` |
| `the_core_spiral_on_a_disc_is_one_piece_with_monotone_radius` (C4 `core_spiral`), `the_curvature_spacing_law_puts_each_tight_pass_on_the_ceiling` (C5), `the_transition_join_nests_its_sectors_on_a_disc` (C6, was ignored), `the_lead_in_radius_holds_the_ceiling_on_a_straight_band`, `the_pivot_frame_lead_in_holds_the_ceiling_on_curved_fronts` (C8), `the_lobe_morph_on_a_rectangle_lobe_holds_one_stepover` (C4), `the_arc_end_lens_matches_a_raster_of_the_end`, `the_wall_normal_points_into_the_nearest_wall`, `the_head_on_rule_keeps_every_turn_bounded_and_no_other_step_over` (C23), `the_pick_keeps_the_plan_the_replay_re_enters_less` (C28), `the_reseeded_field_grows_from_the_cleared_area` (C9), `the_far_side_zone_lies_behind_the_island` (C11) | deleted: the subject is deleted (§2) |
| C22a `a_link_over_a_dropped_group_is_refused_after_the_filter` | set aside in 7.3a; it returns with `slice.rs` in 7.4 |

Test helpers deleted with their tests: `s_prime`, `circular_width`,
`rectangle_lobe`, `segments_cross`, `spacing`, `field_node` (research copy),
`raster_end_lens`, `flat`.

## 4. Gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | ok |
| full clippy (heavy-tests, research, test-support) | ok, 0 warnings |
| `--lib adaptive::` (no `research`) | 71/71 (the 7 moved tests included) |
| `--features research --lib adaptive::geodesic adaptive::research_geodesic` | 14/14, 1 ignored (G-MOPLOAD, as before) |
| `the_geodesic_preset_keeps_its_plan_s7_3` | 2/2: the R pins and the door against the reference |
| `adaptive_geodesic_2d_renders_g1 -- --ignored` (the instrument, run once) | 1/1 in 58.5 s; its shipped columns hold product parity |
| `adaptive3d_emission_byte_parity`, `every_op_keeps_its_toolpath_g_opgolden` | 5/5, 1/1 |
| adaptive sentries: `g_adaptlinkload`, `g_adaptorder`, `adaptive_property_harness`, `contour_spiral_gcode_validity_phase0`, `g_adaptpieces` | ok |
| `folder_maps_name_real_files_f140` | 4/4 |

No product code changed in 7.3d except the `research`-gated door and
the crate visibility of four items, so byte parity holds by construction;
the parity sentries confirm it.
