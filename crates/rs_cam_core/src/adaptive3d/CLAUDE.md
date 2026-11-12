# `adaptive3d/` — 3D adaptive clearing (entry `adaptive_3d_toolpath_output_with_cancel`)

## Files

- `clearing.rs` Z-level engine, regions, `AreaMask` (By Area). The slice
  (`clear_z_level_slice`, CUT-09): `detect_and_order_regions`, `clear_one_region` into a
  `LevelSink` (new per-region work here), `coalesce_level_entries` (check).
- `path.rs` Z levels, level loop, emitter; `search.rs` clear-path test;
  `centre_clip.rs` session boundary clip (before each stamp); `mod.rs`
  facade, `ClearingStrategy3d`; `outer_band.rs` the arm's boundary reach and
  region clip (G-CLIPAIR); `rest_regions.rs` the regions left to a rest op and their top (D44, D48, D50); `level_contour.rs` C4 "Contour each level" (`linking.level_contour`, off: today's plan): the reach band of a rest rough, its contour per level on the containment (read and lowered along the line the emitter ships; closed rings start at a fixed point), the `retract_saves_time` merge and the `FeedCapStart` load cap; C4b `keeps_to_region`, the By Area per-region-level wall mask (F9); `keep_down.rs` the ONE 3D keep-down read
  (`corridor_ceiling_z`, `keep_down_excess`) and the router; `rest_region_filter.rs` the ONE rest region filter (`linking.rest_region_filter`; `build_material_bool_grid` calls it, so both strategies and the CP cleanup skip a region over the area or width bound, whole; the session sets it only on remaining stock); `region_map.rs` By Area map; `intended_floor.rs` the ONE level floor (`level_floor_z`, `plan_bottom_z`; G-ROUGHCOV A3 reads it; never inline it).

## Invariants

- `ClearingStrategy3d` is `ContourParallel` and `GeodesicAdaptive` (removal R1-R6). `GeodesicAdaptive` plans each slice piece with
  `../adaptive/geodesic`; `FeedCapStart`/`End` markers carry each cap to its cut. The F-038 forecast keeps the 2D agent planner.
- `trochoid_cap_mult` and `engagement_measure` are load-only: no strategy reads them. P29-8: `rough_3d` builds `EntryStyle3d` from `feeds::entry_rule::ResolvedEntry`; a bore with no room ramps at the op's own angle (`EntryBore::Fallback::ramp_angle_deg`; 8b: stated, or the rule of the stock class). G-CENTRECUT: `path::effective_entry_style` turns Plunge into the shared ramp at the tool's limit angle for a non-centre-cutting tool; the emitters clamp the rest. `segments_to_toolpath_checked` hands an entry refusal to `Produced`. K5 (P7 C): `clamp_descents_to_limit` (in `emitted_cut_path` and `emitted_link_path`, so the emitter and the stamp agree) raises each point of a `No` tool's cut or link by `CoreZone::clamp_descents` (the straight-line limit per segment and the dwell rule of a turning path, P22-0c-2), only up; the stay-down link's last step ends above the material. Sentry `--test a_rough_plans_its_cut_and_link_descents_to_the_core_limit_k5_centrecut`.
- `max_stay_down_distance_mm` is the ONE stay-down dial (CUT-05, unset 8 x D). Entries
  read floor and proof from the planner stock (`plan_entry`); no link feeds down into stock.
- `Adaptive3dParams` groups (CUT-04): `geometry`, `depth`, `linking`; a new dial joins its group. `linking.step_split` / `keep_down_route` `None` keep today's rule (`PathShape`, `Off`; the geodesic arm: `StockDepth`, `CutFeed`); a route replaces the travel, never the entry's cut (§16.7); `StockDepth` reads `chip_load_fraction`.
- The planner stamps the path that ships (G-SIMCORE: `path::PlannerLuts`, cuts and links stamp the ring, entries the envelope): it applies the segment merge
  before the stamp and the dressups do not merge again (G-PLANSIMGAP). It
  stamps an entry only when a cut commits it; a replaced entry is never
  stamped, and cuts and links stamp from `PlannerCursor::tool_pos`
  (G-PHANTOMSTAMP). Never delete a segment after it is stamped. Every
  segment passes `centre_clip` before its push (G-BOUNDARYPHANTOM). The RDP rule is ONE function, `Adaptive3dGeometry::chord_tolerance`; the model gouge guard reads it (G-ROUGHGOUGE). The shipped cut and link are ONE pipeline each (`path::emitted_cut_path`, `emitted_link_path`: drape, RDP, blend, then `lift_chords_to_leave` with the guard's reduced tool `Adaptive3dGeometry::gouge_probe`, G-DRAPEGAP; the drape alone misses a peak beside the path); the emitter and the stamp both call them.
- A region is in tool-centre space; `planner_outlines` splits it into 2D pieces (D2, G-ADAPT3DPIECES). The geodesic arm opens a region on its grown outline (`grown_outlines`, C31); the full-slot perimeter sweep stays only where the split, the measured margin or the closing guard refuses, or a clip (stock box, centre reach) leaves nothing. P22f-1: no nominal-margin check; `Produced::region_openings` records each region-level's case (sentry `a_geodesic_slice_cuts_no_full_slot_sweep_p22f`). P22f-2a: every fed step the shape rule does not flag is read by `step_load` too; over `c` up to a slot it is capped (`FeedCapSource::StockRead3d`, unless the open 2D cap binds), over a slot split; test door `debug_level_plan_probe` (sentry `a_geodesic_arc_end_reads_the_stock_p22f2`). `Produced::rest_regions` are tool-centre too, inside `centre_boundary` (D48); published only on demand. `depth.rest_top_z` caps only the START of the plan, never the floor or one Depth/Pass (D50).
- ONE Depth/Pass Z plan (`path.rs::step_levels`, plus Detect Flat shelves); every level drapes; no step ladder (operator, 2026-09-24).

## Sentries: `cargo test -p rs_cam_core -q --features test-support --test <name>`

`adaptive3d_boundary_clear_parity`, `adaptive3d_keep_down_link_f038b`,
`adaptive3d_entry_stock_aware`, `adaptive3d_entry_coalescing_f038`,
`agent_search_coverage`, `adaptive3d_subtool_channel_gouge`,
`adaptive3d_global_gate_drapes_below_floor`, `adaptive3d_plan_matches_merged_path_g_plansimgap`,
`adaptive3d_planner_never_ahead_of_emitted_path`, `adaptive3d_emission_byte_parity`, `adaptive3d_boundary_clip_leaves_no_phantom_stock_g_boundaryphantom`,
`adaptive3d_plans_every_slice_piece_g_adapt3dpieces`, `--lib a_chord_beside_a_peak_holds_the_leave_g_drapegap` (the chord lift; red without it), `geodesic_adaptive_keeps_the_research_geometry_s7_4`, `geodesic_adaptive_holds_the_pass_load_c12` (the judge; `--ignored` heavy, about 9 min); rest regions: `a_rough_publishes_the_regions_it_leaves_d44`,
`a_derived_rest_rough_cuts_only_the_leftovers_d44`, `a_rest_op_holds_one_depth_at_the_band_c4`, `contour_each_level_is_off_by_default_c4`, `by_area_walls_cut_each_region_level_c4b`, `--lib _c4`, `a_rest_consumer_stays_inside_the_source_boundary_d48`, `..._starts_at_the_left_stock_top_d50`; D55 (G-AIRPECK) `a_rest_entry_does_not_peck_through_cleared_air_d55` and `--lib _d55` (the peck ladder starts at the contact read, as the helix and the ramp do; real stock still pecks); By Area:
`adaptive3d_by_area_cells_confine_jobs`, `..._by_area_matches_global_stock`; rest filter: `--lib rest_filter` (red without the filter); `--lib adaptive3d::keep_down`, `--lib adaptive3d::tests::the_step_reads_tell_a_full_pass_from_a_thin_layer`.
