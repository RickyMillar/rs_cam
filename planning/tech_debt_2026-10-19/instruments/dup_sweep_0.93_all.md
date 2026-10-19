# Semantic duplicate sweep — rs_cam

Threshold: cosine >= 0.93 · chunks: 14683 (rust, >= 240 chars, inline and out-of-line `#[cfg(test)]` modules dropped) · clusters: 68

src pairs: 13 · test pairs: 388

A pair counts as a test pair when either side lives under `tests/`, `benches/` or `src/bin/`, or is an out-of-line `#[cfg(test)]` module file under `src/`.

Near-duplicate candidates across different files. Semantic similarity, not proof: shared boilerplate, trait impls with the same shape, and intentional parallels all appear here.

---

## Cluster 1 — top score 0.9974 (16 files, 176 pairs)

Files:
- `crates/rs_cam_core/src/metrology/monge.rs`
- `crates/rs_cam_core/src/surface/rest_field.rs`
- `crates/rs_cam_core/tests/banded_raster_costed_f2.rs`
- `crates/rs_cam_core/tests/catchment_basin_census_w0.rs`
- `crates/rs_cam_core/tests/conformal_spiral_synthetic_f2.rs`
- `crates/rs_cam_core/tests/direction_field_wanaka_f1.rs`
- `crates/rs_cam_core/tests/graded_raster_e1.rs`
- `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs`
- `crates/rs_cam_core/tests/spiral_finish_compact_c1.rs`
- `crates/rs_cam_core/tests/thin_organic_island_widths.rs`
- `crates/rs_cam_core/tests/valley_branch_falsifier_h1.rs`
- `crates/rs_cam_core/tests/valley_prize_census_h0.rs`
- `crates/rs_cam_core/tests/wanaka_curvature_anisotropy.rs`
- `crates/rs_cam_core/tests/wanaka_region_capture_f1.rs`
- `crates/rs_cam_core/tests/whole_board_spiral_ledger_g1.rs`
- `crates/rs_cam_core/tests/zone_coherence_census.rs`

Strongest pairs:

- **0.9974** `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs` L209-238 ↔ `crates/rs_cam_core/tests/graded_raster_e1.rs` L220-249
- **0.9966** `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs` L429-480 ↔ `crates/rs_cam_core/tests/graded_raster_e1.rs` L450-501
- **0.9966** `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs` L395-404 ↔ `crates/rs_cam_core/tests/graded_raster_e1.rs` L416-425
- **0.9964** `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs` L620-658 ↔ `crates/rs_cam_core/tests/graded_raster_e1.rs` L594-632
- **0.9964** `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs` L405-428 ↔ `crates/rs_cam_core/tests/graded_raster_e1.rs` L426-449
- … 171 more pairs

## Cluster 2 — top score 0.9898 (7 files, 16 pairs)

Files:
- `crates/rs_cam_core/tests/hatches_are_crate_private_wp7.rs`
- `crates/rs_cam_core/tests/setters_are_crate_private_wp15b.rs`
- `crates/rs_cam_core/tests/setters_have_rows_wp15a.rs`
- `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs`
- `crates/rs_cam_viz/tests/non_egui_sites_write_through_commands_wp6b.rs`
- `crates/rs_cam_viz/tests/panels_read_the_token_module_up1.rs`
- `crates/rs_cam_viz/tests/ui_string_hygiene.rs`

Strongest pairs:

- **0.9898** `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs` L96-112 ↔ `crates/rs_cam_viz/tests/non_egui_sites_write_through_commands_wp6b.rs` L121-137
- **0.9896** `crates/rs_cam_viz/tests/non_egui_sites_write_through_commands_wp6b.rs` L138-148 ↔ `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs` L113-123
- **0.9740** `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs` L124-161 ↔ `crates/rs_cam_viz/tests/non_egui_sites_write_through_commands_wp6b.rs` L149-192
- **0.9732** `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs` L192-229 ↔ `crates/rs_cam_viz/tests/non_egui_sites_write_through_commands_wp6b.rs` L225-258
- **0.9618** `crates/rs_cam_core/tests/setters_have_rows_wp15a.rs` L242-258 ↔ `crates/rs_cam_core/tests/setters_are_crate_private_wp15b.rs` L244-260
- … 11 more pairs

## Cluster 3 — top score 0.9887 (10 files, 39 pairs)

Files:
- `crates/rs_cam_core/tests/classification_columns_ab_m3.rs`
- `crates/rs_cam_core/tests/common/meshes.rs`
- `crates/rs_cam_core/tests/common/tools.rs`
- `crates/rs_cam_core/tests/crease_own_region_pr6b.rs`
- `crates/rs_cam_core/tests/derived_stepover_pr6a.rs`
- `crates/rs_cam_core/tests/inert_claims_dial_f4.rs`
- `crates/rs_cam_core/tests/strategy_comparison_h4.rs`
- `crates/rs_cam_core/tests/unified_finish_dropped_band_finding_d1.rs`
- `crates/rs_cam_core/tests/unified_finish_tapered_end_to_end_m21.rs`
- `crates/rs_cam_viz/tests/a_tool_size_reads_diameter_and_radius_g_toolsize.rs`

Strongest pairs:

- **0.9887** `crates/rs_cam_core/tests/crease_own_region_pr6b.rs` L117-135 ↔ `crates/rs_cam_core/tests/derived_stepover_pr6a.rs` L114-132
- **0.9805** `crates/rs_cam_core/tests/derived_stepover_pr6a.rs` L96-113 ↔ `crates/rs_cam_core/tests/crease_own_region_pr6b.rs` L99-116
- **0.9776** `crates/rs_cam_core/tests/crease_own_region_pr6b.rs` L83-91 ↔ `crates/rs_cam_core/tests/derived_stepover_pr6a.rs` L80-88
- **0.9767** `crates/rs_cam_core/tests/classification_columns_ab_m3.rs` L149-190 ↔ `crates/rs_cam_core/tests/strategy_comparison_h4.rs` L330-369
- **0.9738** `crates/rs_cam_core/tests/inert_claims_dial_f4.rs` L99-113 ↔ `crates/rs_cam_core/tests/unified_finish_dropped_band_finding_d1.rs` L104-118
- … 34 more pairs

## Cluster 4 — top score 0.9885 (2 files, 4 pairs)

Files:
- `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs`
- `crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs`

Strongest pairs:

- **0.9885** `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs` L73-88 ↔ `crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs` L64-79
- **0.9857** `crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs` L80-94 ↔ `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs` L89-103
- **0.9838** `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs` L104-127 ↔ `crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs` L95-118
- **0.9648** `crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs` L127-145 ↔ `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs` L136-152

## Cluster 5 — top score 0.9863 (4 files, 3 pairs)

Files:
- `crates/rs_cam_viz/tests/inspector_header_wraps_g_reachwrap.rs`
- `crates/rs_cam_viz/tests/inspector_width_is_tab_independent_up4.rs`
- `crates/rs_cam_viz/tests/the_inspector_nests_once_dc5.rs`
- `crates/rs_cam_viz/tests/workspace_menu_complete_g_wsmenu.rs`

Strongest pairs:

- **0.9863** `crates/rs_cam_viz/tests/inspector_width_is_tab_independent_up4.rs` L169-186 ↔ `crates/rs_cam_viz/tests/the_inspector_nests_once_dc5.rs` L203-221
- **0.9509** `crates/rs_cam_viz/tests/inspector_width_is_tab_independent_up4.rs` L64-83 ↔ `crates/rs_cam_viz/tests/inspector_header_wraps_g_reachwrap.rs` L87-106
- **0.9328** `crates/rs_cam_viz/tests/inspector_width_is_tab_independent_up4.rs` L64-83 ↔ `crates/rs_cam_viz/tests/workspace_menu_complete_g_wsmenu.rs` L55-74

## Cluster 6 — top score 0.9841 (2 files, 4 pairs)

Files:
- `crates/rs_cam_core/tests/entry_descent_profile_b2.rs`
- `crates/rs_cam_core/tests/profile_link_ceiling.rs`

Strongest pairs:

- **0.9841** `crates/rs_cam_core/tests/entry_descent_profile_b2.rs` L135-153 ↔ `crates/rs_cam_core/tests/profile_link_ceiling.rs` L117-135
- **0.9839** `crates/rs_cam_core/tests/entry_descent_profile_b2.rs` L167-175 ↔ `crates/rs_cam_core/tests/profile_link_ceiling.rs` L153-161
- **0.9418** `crates/rs_cam_core/tests/profile_link_ceiling.rs` L136-152 ↔ `crates/rs_cam_core/tests/entry_descent_profile_b2.rs` L154-166
- **0.9415** `crates/rs_cam_core/tests/profile_link_ceiling.rs` L95-104 ↔ `crates/rs_cam_core/tests/entry_descent_profile_b2.rs` L121-134

## Cluster 7 — top score 0.9796 (7 files, 13 pairs)

Files:
- `crates/rs_cam_core/tests/adopt_simulation_stores_prior_stocks.rs`
- `crates/rs_cam_core/tests/feed_optimization_refusals_wp18.rs`
- `crates/rs_cam_core/tests/gen_inputs_one_assembly_n12.rs`
- `crates/rs_cam_core/tests/job_three_steps_equal_generate_toolpath.rs`
- `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs`
- `crates/rs_cam_core/tests/resolved_gen_inputs_has_one_producer.rs`
- `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs`

Strongest pairs:

- **0.9796** `crates/rs_cam_core/tests/gen_inputs_one_assembly_n12.rs` L106-122 ↔ `crates/rs_cam_core/tests/feed_optimization_refusals_wp18.rs` L251-268
- **0.9676** `crates/rs_cam_core/tests/adopt_simulation_stores_prior_stocks.rs` L68-99 ↔ `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs` L85-115
- **0.9664** `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs` L116-138 ↔ `crates/rs_cam_core/tests/adopt_simulation_stores_prior_stocks.rs` L100-122
- **0.9659** `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs` L93-107 ↔ `crates/rs_cam_core/tests/gen_inputs_one_assembly_n12.rs` L228-242
- **0.9639** `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs` L328-363 ↔ `crates/rs_cam_core/tests/feed_optimization_refusals_wp18.rs` L447-482
- … 8 more pairs

## Cluster 8 — top score 0.9739 (2 files, 3 pairs)

Files:
- `crates/rs_cam_core/tests/band_stamping_determinism_s3.rs`
- `crates/rs_cam_core/tests/swept_stamping_s1.rs`

Strongest pairs:

- **0.9739** `crates/rs_cam_core/tests/band_stamping_determinism_s3.rs` L110-136 ↔ `crates/rs_cam_core/tests/swept_stamping_s1.rs` L147-173
- **0.9633** `crates/rs_cam_core/tests/band_stamping_determinism_s3.rs` L78-109 ↔ `crates/rs_cam_core/tests/swept_stamping_s1.rs` L81-112
- **0.9538** `crates/rs_cam_core/tests/band_stamping_determinism_s3.rs` L137-202 ↔ `crates/rs_cam_core/tests/swept_stamping_s1.rs` L174-228

## Cluster 9 — top score 0.9721 (3 files, 4 pairs)

Files:
- `crates/rs_cam_core/src/maps/finish_surface_cache.rs`
- `crates/rs_cam_core/src/maps/reach_map_cache.rs`
- `crates/rs_cam_core/src/maps/tier_map_cache.rs`

Strongest pairs:

- **0.9721** `crates/rs_cam_core/src/maps/tier_map_cache.rs` L187-200 ↔ `crates/rs_cam_core/src/maps/reach_map_cache.rs` L184-197
- **0.9538** `crates/rs_cam_core/src/maps/tier_map_cache.rs` L259-267 ↔ `crates/rs_cam_core/src/maps/reach_map_cache.rs` L246-254
- **0.9414** `crates/rs_cam_core/src/maps/finish_surface_cache.rs` L306-318 ↔ `crates/rs_cam_core/src/maps/tier_map_cache.rs` L187-200
- **0.9358** `crates/rs_cam_core/src/maps/finish_surface_cache.rs` L306-318 ↔ `crates/rs_cam_core/src/maps/reach_map_cache.rs` L184-197

## Cluster 10 — top score 0.9714 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp.rs`
- `crates/rs_cam_core/tests/a_ramp_off_the_g6_claim_holds_its_vertical_rate_at_the_plunge_g10.rs`

Strongest pairs:

- **0.9714** `crates/rs_cam_core/tests/a_ramp_off_the_g6_claim_holds_its_vertical_rate_at_the_plunge_g10.rs` L74-82 ↔ `crates/rs_cam_core/tests/a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp.rs` L133-141

## Cluster 11 — top score 0.9708 (2 files, 3 pairs)

Files:
- `crates/rs_cam_core/tests/the_research_arms_are_feature_gated_fin01.rs`
- `crates/rs_cam_core/tests/the_test_doors_are_gated_fld0405.rs`

Strongest pairs:

- **0.9708** `crates/rs_cam_core/tests/the_test_doors_are_gated_fld0405.rs` L255-272 ↔ `crates/rs_cam_core/tests/the_research_arms_are_feature_gated_fin01.rs` L103-121
- **0.9647** `crates/rs_cam_core/tests/the_research_arms_are_feature_gated_fin01.rs` L145-152 ↔ `crates/rs_cam_core/tests/the_test_doors_are_gated_fld0405.rs` L296-303
- **0.9526** `crates/rs_cam_core/tests/the_research_arms_are_feature_gated_fin01.rs` L122-144 ↔ `crates/rs_cam_core/tests/the_test_doors_are_gated_fld0405.rs` L273-295

## Cluster 12 — top score 0.9698 (2 files, 4 pairs)

Files:
- `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs`
- `crates/rs_cam_core/tests/retime_respects_no_kinematics_n7.rs`

Strongest pairs:

- **0.9698** `crates/rs_cam_core/tests/retime_respects_no_kinematics_n7.rs` L111-130 ↔ `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs` L100-119
- **0.9580** `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs` L146-180 ↔ `crates/rs_cam_core/tests/retime_respects_no_kinematics_n7.rs` L157-190
- **0.9401** `crates/rs_cam_core/tests/retime_respects_no_kinematics_n7.rs` L191-229 ↔ `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs` L181-219
- **0.9304** `crates/rs_cam_core/tests/retime_respects_no_kinematics_n7.rs` L394-438 ↔ `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs` L383-437

## Cluster 13 — top score 0.9685 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/steep_shallow_min_segment_pr8d.rs`
- `crates/rs_cam_core/tests/waterline_shared_finish_setup_c3.rs`

Strongest pairs:

- **0.9685** `crates/rs_cam_core/tests/steep_shallow_min_segment_pr8d.rs` L116-134 ↔ `crates/rs_cam_core/tests/waterline_shared_finish_setup_c3.rs` L102-121

## Cluster 14 — top score 0.9684 (4 files, 8 pairs)

Files:
- `crates/rs_cam_core/tests/air_filter_tool_aware_s3.rs`
- `crates/rs_cam_core/tests/rapid_check_catches_real_side_strikes_g_rapid6497.rs`
- `crates/rs_cam_core/tests/rapid_check_catches_shallow_plunges_g_rapidplungetol.rs`
- `crates/rs_cam_core/tests/rapid_live_check_crest_s2.rs`

Strongest pairs:

- **0.9684** `crates/rs_cam_core/tests/rapid_check_catches_real_side_strikes_g_rapid6497.rs` L114-127 ↔ `crates/rs_cam_core/tests/rapid_check_catches_shallow_plunges_g_rapidplungetol.rs` L113-126
- **0.9613** `crates/rs_cam_core/tests/rapid_check_catches_shallow_plunges_g_rapidplungetol.rs` L88-112 ↔ `crates/rs_cam_core/tests/rapid_check_catches_real_side_strikes_g_rapid6497.rs` L79-104
- **0.9535** `crates/rs_cam_core/tests/rapid_check_catches_shallow_plunges_g_rapidplungetol.rs` L182-204 ↔ `crates/rs_cam_core/tests/rapid_live_check_crest_s2.rs` L131-157
- **0.9463** `crates/rs_cam_core/tests/rapid_live_check_crest_s2.rs` L94-130 ↔ `crates/rs_cam_core/tests/rapid_check_catches_real_side_strikes_g_rapid6497.rs` L134-166
- **0.9453** `crates/rs_cam_core/tests/rapid_check_catches_shallow_plunges_g_rapidplungetol.rs` L76-87 ↔ `crates/rs_cam_core/tests/rapid_check_catches_real_side_strikes_g_rapid6497.rs` L66-78
- … 3 more pairs

## Cluster 15 — top score 0.9682 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/export_datum_setup_frame.rs`
- `crates/rs_cam_core/tests/export_honors_coolant_p0d1.rs`
- `crates/rs_cam_viz/tests/export_parity_core_vs_gui_p0.rs`

Strongest pairs:

- **0.9682** `crates/rs_cam_viz/tests/export_parity_core_vs_gui_p0.rs` L396-455 ↔ `crates/rs_cam_core/tests/export_datum_setup_frame.rs` L219-276
- **0.9627** `crates/rs_cam_core/tests/export_honors_coolant_p0d1.rs` L151-163 ↔ `crates/rs_cam_viz/tests/export_parity_core_vs_gui_p0.rs` L375-387

## Cluster 16 — top score 0.9665 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/chipload_thinning_magnitude_survey.rs`
- `crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs`

Strongest pairs:

- **0.9665** `crates/rs_cam_core/tests/chipload_thinning_magnitude_survey.rs` L169-196 ↔ `crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs` L199-224

## Cluster 17 — top score 0.9652 (2 files, 4 pairs)

Files:
- `crates/rs_cam_core/tests/chamfer_bevel_sits_on_the_edge_g_chamferside.rs`
- `crates/rs_cam_core/tests/chamfer_inside_edge_bevels_into_the_material_g_chamferinside.rs`

Strongest pairs:

- **0.9652** `crates/rs_cam_core/tests/chamfer_inside_edge_bevels_into_the_material_g_chamferinside.rs` L245-270 ↔ `crates/rs_cam_core/tests/chamfer_bevel_sits_on_the_edge_g_chamferside.rs` L117-142
- **0.9549** `crates/rs_cam_core/tests/chamfer_inside_edge_bevels_into_the_material_g_chamferinside.rs` L284-345 ↔ `crates/rs_cam_core/tests/chamfer_bevel_sits_on_the_edge_g_chamferside.rs` L153-216
- **0.9490** `crates/rs_cam_core/tests/chamfer_inside_edge_bevels_into_the_material_g_chamferinside.rs` L271-283 ↔ `crates/rs_cam_core/tests/chamfer_bevel_sits_on_the_edge_g_chamferside.rs` L143-152
- **0.9332** `crates/rs_cam_core/tests/chamfer_inside_edge_bevels_into_the_material_g_chamferinside.rs` L346-412 ↔ `crates/rs_cam_core/tests/chamfer_bevel_sits_on_the_edge_g_chamferside.rs` L217-265

## Cluster 18 — top score 0.9651 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/drill_no_targets_refuses_g_drillcentroid.rs`
- `crates/rs_cam_core/tests/drill_pick_emission_frame_g_drillpick.rs`
- `crates/rs_cam_core/tests/drill_picks_resolve_to_targets_g_drillpickstale.rs`

Strongest pairs:

- **0.9651** `crates/rs_cam_core/tests/drill_no_targets_refuses_g_drillcentroid.rs` L144-158 ↔ `crates/rs_cam_core/tests/drill_picks_resolve_to_targets_g_drillpickstale.rs` L174-188
- **0.9414** `crates/rs_cam_core/tests/drill_no_targets_refuses_g_drillcentroid.rs` L159-184 ↔ `crates/rs_cam_core/tests/drill_pick_emission_frame_g_drillpick.rs` L218-245

## Cluster 19 — top score 0.9650 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/src/ui/sim_op_list.rs`

Strongest pairs:

- **0.9650** `crates/rs_cam_viz/src/ui/sim_op_list.rs` L1248-1258 ↔ `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` L999-1009

## Cluster 20 — top score 0.9617 (11 files, 30 pairs)

Files:
- `crates/rs_cam_viz/tests/a_click_picks_the_drawn_item_p2_6.rs`
- `crates/rs_cam_viz/tests/add_toolpath_via_gui_g_guiadd.rs`
- `crates/rs_cam_viz/tests/an_unbacked_cell_still_adds_its_operation_fm5.rs`
- `crates/rs_cam_viz/tests/apply_contract_a3.rs`
- `crates/rs_cam_viz/tests/feeds_apply_drops_result_n13.rs`
- `crates/rs_cam_viz/tests/generate_all_fixpoint_parity.rs`
- `crates/rs_cam_viz/tests/get_notifications_g_toastread.rs`
- `crates/rs_cam_viz/tests/mcp_toasts_report_outcome_g_mcptoast.rs`
- `crates/rs_cam_viz/tests/the_chipload_verdict_is_one_row_g_chipverdict.rs`
- `crates/rs_cam_viz/tests/the_recommended_column_is_what_apply_writes_g_recomapplied.rs`
- `crates/rs_cam_viz/tests/the_speeds_apply_holds_the_cut_g_speedsonly.rs`

Strongest pairs:

- **0.9617** `crates/rs_cam_viz/tests/apply_contract_a3.rs` L56-73 ↔ `crates/rs_cam_viz/tests/the_recommended_column_is_what_apply_writes_g_recomapplied.rs` L69-86
- **0.9576** `crates/rs_cam_viz/tests/get_notifications_g_toastread.rs` L138-155 ↔ `crates/rs_cam_viz/tests/the_recommended_column_is_what_apply_writes_g_recomapplied.rs` L69-86
- **0.9536** `crates/rs_cam_viz/tests/get_notifications_g_toastread.rs` L138-155 ↔ `crates/rs_cam_viz/tests/apply_contract_a3.rs` L56-73
- **0.9528** `crates/rs_cam_viz/tests/the_recommended_column_is_what_apply_writes_g_recomapplied.rs` L69-86 ↔ `crates/rs_cam_viz/tests/feeds_apply_drops_result_n13.rs` L74-91
- **0.9478** `crates/rs_cam_viz/tests/apply_contract_a3.rs` L56-73 ↔ `crates/rs_cam_viz/tests/feeds_apply_drops_result_n13.rs` L74-91
- … 25 more pairs

## Cluster 21 — top score 0.9613 (3 files, 6 pairs)

Files:
- `crates/rs_cam_core/tests/a_lifted_link_lands_as_an_entry_b20.rs`
- `crates/rs_cam_core/tests/a_link_between_regions_clears_standing_stock_b16.rs`
- `crates/rs_cam_core/tests/island_stay_down_links_o3.rs`

Strongest pairs:

- **0.9613** `crates/rs_cam_core/tests/a_link_between_regions_clears_standing_stock_b16.rs` L151-165 ↔ `crates/rs_cam_core/tests/island_stay_down_links_o3.rs` L130-144
- **0.9514** `crates/rs_cam_core/tests/a_link_between_regions_clears_standing_stock_b16.rs` L46-80 ↔ `crates/rs_cam_core/tests/a_lifted_link_lands_as_an_entry_b20.rs` L49-84
- **0.9449** `crates/rs_cam_core/tests/island_stay_down_links_o3.rs` L57-89 ↔ `crates/rs_cam_core/tests/a_lifted_link_lands_as_an_entry_b20.rs` L49-84
- **0.9434** `crates/rs_cam_core/tests/a_link_between_regions_clears_standing_stock_b16.rs` L81-93 ↔ `crates/rs_cam_core/tests/island_stay_down_links_o3.rs` L90-103
- **0.9399** `crates/rs_cam_core/tests/a_link_between_regions_clears_standing_stock_b16.rs` L108-117 ↔ `crates/rs_cam_core/tests/island_stay_down_links_o3.rs` L119-129
- … 1 more pairs

## Cluster 22 — top score 0.9607 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/checkpoint_a_valley_matrix.rs`
- `crates/rs_cam_core/tests/checkpoint_c9_sampled_reach.rs`

Strongest pairs:

- **0.9607** `crates/rs_cam_core/tests/checkpoint_c9_sampled_reach.rs` L467-477 ↔ `crates/rs_cam_core/tests/checkpoint_a_valley_matrix.rs` L861-871

## Cluster 23 — top score 0.9595 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/feeds/extrapolation/plunge.rs`
- `crates/rs_cam_core/src/feeds/plunge.rs`

Strongest pairs:

- **0.9595** `crates/rs_cam_core/src/feeds/extrapolation/plunge.rs` L345-353 ↔ `crates/rs_cam_core/src/feeds/plunge.rs` L246-254

## Cluster 24 — top score 0.9591 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/isoclip_entry_ramp_g_isoclipentry.rs`
- `crates/rs_cam_core/tests/isoclip_link_rapid_g_isocliprapid.rs`

Strongest pairs:

- **0.9591** `crates/rs_cam_core/tests/isoclip_entry_ramp_g_isoclipentry.rs` L125-149 ↔ `crates/rs_cam_core/tests/isoclip_link_rapid_g_isocliprapid.rs` L110-132

## Cluster 25 — top score 0.9580 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/finish_resolution_policy_pr3.rs`
- `crates/rs_cam_core/tests/finish_surface_cache.rs`

Strongest pairs:

- **0.9580** `crates/rs_cam_core/tests/finish_resolution_policy_pr3.rs` L114-141 ↔ `crates/rs_cam_core/tests/finish_surface_cache.rs` L66-95

## Cluster 26 — top score 0.9577 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/op_model_ref_static_validation_f023.rs`
- `crates/rs_cam_core/tests/op_precondition_static_validation_f015.rs`

Strongest pairs:

- **0.9577** `crates/rs_cam_core/tests/op_model_ref_static_validation_f023.rs` L51-67 ↔ `crates/rs_cam_core/tests/op_precondition_static_validation_f015.rs` L49-65
- **0.9302** `crates/rs_cam_core/tests/op_precondition_static_validation_f015.rs` L103-130 ↔ `crates/rs_cam_core/tests/op_model_ref_static_validation_f023.rs` L68-95

## Cluster 27 — top score 0.9576 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/checkpoint_mesh_ab_m2.rs`
- `crates/rs_cam_core/tests/checkpoint_mesh_on_demand_m2.rs`
- `crates/rs_cam_core/tests/sim_prefix_memo_s5.rs`

Strongest pairs:

- **0.9576** `crates/rs_cam_core/tests/checkpoint_mesh_ab_m2.rs` L96-106 ↔ `crates/rs_cam_core/tests/sim_prefix_memo_s5.rs` L235-248
- **0.9434** `crates/rs_cam_core/tests/checkpoint_mesh_on_demand_m2.rs` L97-114 ↔ `crates/rs_cam_core/tests/checkpoint_mesh_ab_m2.rs` L78-95

## Cluster 28 — top score 0.9567 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/feed_explanation_record_t1.rs`
- `crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs`

Strongest pairs:

- **0.9567** `crates/rs_cam_core/tests/feed_explanation_record_t1.rs` L120-172 ↔ `crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs` L552-607

## Cluster 29 — top score 0.9565 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/compute/operation_configs.rs`
- `crates/rs_cam_core/src/ops/project_curve.rs`

Strongest pairs:

- **0.9565** `crates/rs_cam_core/src/compute/operation_configs.rs` L2558-2573 ↔ `crates/rs_cam_core/src/ops/project_curve.rs` L48-65

## Cluster 30 — top score 0.9560 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/dressup_span_invariants.rs`
- `crates/rs_cam_core/tests/transform_provenance_fingerprints.rs`

Strongest pairs:

- **0.9560** `crates/rs_cam_core/tests/dressup_span_invariants.rs` L157-189 ↔ `crates/rs_cam_core/tests/transform_provenance_fingerprints.rs` L128-163

## Cluster 31 — top score 0.9558 (5 files, 4 pairs)

Files:
- `crates/rs_cam_core/tests/adaptive_feed_modulation_gcode_f036a.rs`
- `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs`
- `crates/rs_cam_core/tests/air_cut_one_time_base_g_airdenom.rs`
- `crates/rs_cam_core/tests/predicted_feed_gates_f035.rs`
- `crates/rs_cam_viz/tests/export_states_the_feed_stage_w2_5.rs`

Strongest pairs:

- **0.9558** `crates/rs_cam_core/tests/adaptive_feed_modulation_gcode_f036a.rs` L45-63 ↔ `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` L251-270
- **0.9523** `crates/rs_cam_core/tests/predicted_feed_gates_f035.rs` L124-134 ↔ `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` L78-87
- **0.9497** `crates/rs_cam_viz/tests/export_states_the_feed_stage_w2_5.rs` L88-104 ↔ `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` L88-106
- **0.9301** `crates/rs_cam_core/tests/air_cut_one_time_base_g_airdenom.rs` L182-194 ↔ `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` L198-210

## Cluster 32 — top score 0.9557 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/an_arc_fit_holds_its_source_in_3d_g_tierburial.rs`
- `crates/rs_cam_core/tests/arcfit_gate_population_d4.rs`

Strongest pairs:

- **0.9557** `crates/rs_cam_core/tests/an_arc_fit_holds_its_source_in_3d_g_tierburial.rs` L102-113 ↔ `crates/rs_cam_core/tests/arcfit_gate_population_d4.rs` L231-242

## Cluster 33 — top score 0.9532 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/multitool_plan_emission_o1.rs`
- `crates/rs_cam_core/tests/multitool_preview_u1.rs`
- `crates/rs_cam_core/tests/planned_tier_regions_boundary_o2.rs`

Strongest pairs:

- **0.9532** `crates/rs_cam_core/tests/multitool_preview_u1.rs` L67-77 ↔ `crates/rs_cam_core/tests/planned_tier_regions_boundary_o2.rs` L91-102
- **0.9303** `crates/rs_cam_core/tests/multitool_preview_u1.rs` L92-105 ↔ `crates/rs_cam_core/tests/multitool_plan_emission_o1.rs` L54-67

## Cluster 34 — top score 0.9526 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/cut_efficiency_is_closed_form_g_specenergy.rs`
- `crates/rs_cam_core/tests/efficiency_abstains_without_kc_g_specenergy.rs`

Strongest pairs:

- **0.9526** `crates/rs_cam_core/tests/efficiency_abstains_without_kc_g_specenergy.rs` L102-112 ↔ `crates/rs_cam_core/tests/cut_efficiency_is_closed_form_g_specenergy.rs` L164-174
- **0.9309** `crates/rs_cam_core/tests/cut_efficiency_is_closed_form_g_specenergy.rs` L175-214 ↔ `crates/rs_cam_core/tests/efficiency_abstains_without_kc_g_specenergy.rs` L113-143

## Cluster 35 — top score 0.9518 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/shallow_raster_slope_derate.rs`
- `crates/rs_cam_core/tests/shipped_raster_spacing_b1.rs`

Strongest pairs:

- **0.9518** `crates/rs_cam_core/tests/shallow_raster_slope_derate.rs` L50-73 ↔ `crates/rs_cam_core/tests/shipped_raster_spacing_b1.rs` L223-253

## Cluster 36 — top score 0.9511 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs`
- `crates/rs_cam_core/tests/adaptive3d_planner_stock_xy_f027.rs`

Strongest pairs:

- **0.9511** `crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs` L84-176 ↔ `crates/rs_cam_core/tests/adaptive3d_planner_stock_xy_f027.rs` L66-167

## Cluster 37 — top score 0.9507 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/feedopt_caps_plunges_g_feedoptplunge.rs`
- `crates/rs_cam_core/tests/feedopt_clamp_never_panics_wp21.rs`

Strongest pairs:

- **0.9507** `crates/rs_cam_core/tests/feedopt_clamp_never_panics_wp21.rs` L145-154 ↔ `crates/rs_cam_core/tests/feedopt_caps_plunges_g_feedoptplunge.rs` L436-445
- **0.9419** `crates/rs_cam_core/tests/feedopt_caps_plunges_g_feedoptplunge.rs` L96-119 ↔ `crates/rs_cam_core/tests/feedopt_clamp_never_panics_wp21.rs` L80-110

## Cluster 38 — top score 0.9493 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/tests/mcp_authoring_surface.rs`
- `crates/rs_cam_viz/tests/mcp_rebind_surface_g_mcprebind.rs`

Strongest pairs:

- **0.9493** `crates/rs_cam_viz/tests/mcp_authoring_surface.rs` L42-54 ↔ `crates/rs_cam_viz/tests/mcp_rebind_surface_g_mcprebind.rs` L37-49

## Cluster 39 — top score 0.9491 (3 files, 4 pairs)

Files:
- `crates/rs_cam_core/tests/command_registry_completeness.rs`
- `crates/rs_cam_core/tests/mcp_mutation_rows_reach_core.rs`
- `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs`

Strongest pairs:

- **0.9491** `crates/rs_cam_core/tests/mcp_mutation_rows_reach_core.rs` L155-189 ↔ `crates/rs_cam_core/tests/command_registry_completeness.rs` L301-334
- **0.9463** `crates/rs_cam_core/tests/mcp_mutation_rows_reach_core.rs` L143-154 ↔ `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs` L206-220
- **0.9432** `crates/rs_cam_core/tests/command_registry_completeness.rs` L286-300 ↔ `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs` L206-220
- **0.9376** `crates/rs_cam_core/tests/command_registry_completeness.rs` L360-379 ↔ `crates/rs_cam_core/tests/mcp_mutation_rows_reach_core.rs` L211-226

## Cluster 40 — top score 0.9489 (2 files, 1 pairs)

Files:
- `crates/rs_cam_cli/src/project.rs`
- `crates/rs_cam_core/src/session/diagnostics_types.rs`

Strongest pairs:

- **0.9489** `crates/rs_cam_core/src/session/diagnostics_types.rs` L25-99 ↔ `crates/rs_cam_cli/src/project.rs` L51-110

## Cluster 41 — top score 0.9486 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/stock/cut_as.rs`
- `crates/rs_cam_core/src/tool_load/verdict.rs`

Strongest pairs:

- **0.9486** `crates/rs_cam_core/src/stock/cut_as.rs` L255-269 ↔ `crates/rs_cam_core/src/tool_load/verdict.rs` L564-576

## Cluster 42 — top score 0.9484 (3 files, 4 pairs)

Files:
- `crates/rs_cam_viz/tests/pin_editor_and_viewport_p7.rs`
- `crates/rs_cam_viz/tests/pin_section_fixes_in_place_r2.rs`
- `crates/rs_cam_viz/tests/pin_wizard_apply_cancel_r4.rs`

Strongest pairs:

- **0.9484** `crates/rs_cam_viz/tests/pin_editor_and_viewport_p7.rs` L46-66 ↔ `crates/rs_cam_viz/tests/pin_section_fixes_in_place_r2.rs` L61-81
- **0.9424** `crates/rs_cam_viz/tests/pin_editor_and_viewport_p7.rs` L160-188 ↔ `crates/rs_cam_viz/tests/pin_wizard_apply_cancel_r4.rs` L112-143
- **0.9377** `crates/rs_cam_viz/tests/pin_section_fixes_in_place_r2.rs` L88-117 ↔ `crates/rs_cam_viz/tests/pin_editor_and_viewport_p7.rs` L73-102
- **0.9342** `crates/rs_cam_viz/tests/pin_wizard_apply_cancel_r4.rs` L47-67 ↔ `crates/rs_cam_viz/tests/pin_editor_and_viewport_p7.rs` L46-66

## Cluster 43 — top score 0.9474 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/every_op_keeps_its_toolpath_g_opgolden.rs`
- `crates/rs_cam_core/tests/the_volumetric_wrapper_keeps_its_produced_w6_5.rs`

Strongest pairs:

- **0.9474** `crates/rs_cam_core/tests/the_volumetric_wrapper_keeps_its_produced_w6_5.rs` L153-161 ↔ `crates/rs_cam_core/tests/every_op_keeps_its_toolpath_g_opgolden.rs` L275-284
- **0.9364** `crates/rs_cam_core/tests/the_volumetric_wrapper_keeps_its_produced_w6_5.rs` L143-152 ↔ `crates/rs_cam_core/tests/every_op_keeps_its_toolpath_g_opgolden.rs` L263-274

## Cluster 44 — top score 0.9471 (3 files, 3 pairs)

Files:
- `crates/rs_cam_core/tests/a_criterion_carries_its_own_bound_g_s4bound.rs`
- `crates/rs_cam_core/tests/an_absent_limit_is_visibly_absent_g_gantry.rs`
- `crates/rs_cam_core/tests/an_unmodelled_refusal_names_every_row_g_t19.rs`

Strongest pairs:

- **0.9471** `crates/rs_cam_core/tests/an_absent_limit_is_visibly_absent_g_gantry.rs` L118-157 ↔ `crates/rs_cam_core/tests/a_criterion_carries_its_own_bound_g_s4bound.rs` L134-173
- **0.9423** `crates/rs_cam_core/tests/an_unmodelled_refusal_names_every_row_g_t19.rs` L186-217 ↔ `crates/rs_cam_core/tests/an_absent_limit_is_visibly_absent_g_gantry.rs` L177-214
- **0.9326** `crates/rs_cam_core/tests/an_absent_limit_is_visibly_absent_g_gantry.rs` L61-89 ↔ `crates/rs_cam_core/tests/a_criterion_carries_its_own_bound_g_s4bound.rs` L73-101

## Cluster 45 — top score 0.9466 (4 files, 3 pairs)

Files:
- `crates/rs_cam_core/tests/a_flat_end_plunge_is_the_side_chip_over_z_g6.rs`
- `crates/rs_cam_core/tests/a_printed_cell_serves_every_family_through_one_claim_g3.rs`
- `crates/rs_cam_core/tests/clueless_cells_refuse_and_backed_cells_ship_fm5.rs`
- `crates/rs_cam_core/tests/the_printed_tip_row_serves_every_3d_finish_g3v8.rs`

Strongest pairs:

- **0.9466** `crates/rs_cam_core/tests/the_printed_tip_row_serves_every_3d_finish_g3v8.rs` L55-65 ↔ `crates/rs_cam_core/tests/a_printed_cell_serves_every_family_through_one_claim_g3.rs` L83-93
- **0.9413** `crates/rs_cam_core/tests/clueless_cells_refuse_and_backed_cells_ship_fm5.rs` L177-188 ↔ `crates/rs_cam_core/tests/a_flat_end_plunge_is_the_side_chip_over_z_g6.rs` L240-251
- **0.9409** `crates/rs_cam_core/tests/clueless_cells_refuse_and_backed_cells_ship_fm5.rs` L251-263 ↔ `crates/rs_cam_core/tests/a_printed_cell_serves_every_family_through_one_claim_g3.rs` L83-93

## Cluster 46 — top score 0.9464 (5 files, 5 pairs)

Files:
- `crates/rs_cam_core/tests/query_cycle_time_one_answer.rs`
- `crates/rs_cam_core/tests/stock_carries_across_setups_s0.rs`
- `crates/rs_cam_core/tests/stock_changes_apply_to_the_stock_s3.rs`
- `crates/rs_cam_core/tests/stock_changes_stale_from_their_setup_on_s2.rs`
- `crates/rs_cam_viz/tests/cycle_time_basis_g_timeest.rs`

Strongest pairs:

- **0.9464** `crates/rs_cam_core/tests/stock_carries_across_setups_s0.rs` L187-197 ↔ `crates/rs_cam_core/tests/stock_changes_apply_to_the_stock_s3.rs` L243-253
- **0.9359** `crates/rs_cam_core/tests/stock_changes_apply_to_the_stock_s3.rs` L204-215 ↔ `crates/rs_cam_core/tests/stock_changes_stale_from_their_setup_on_s2.rs` L242-260
- **0.9352** `crates/rs_cam_viz/tests/cycle_time_basis_g_timeest.rs` L89-111 ↔ `crates/rs_cam_core/tests/query_cycle_time_one_answer.rs` L130-146
- **0.9347** `crates/rs_cam_core/tests/stock_changes_stale_from_their_setup_on_s2.rs` L159-169 ↔ `crates/rs_cam_core/tests/query_cycle_time_one_answer.rs` L209-219
- **0.9314** `crates/rs_cam_core/tests/stock_carries_across_setups_s0.rs` L169-186 ↔ `crates/rs_cam_core/tests/stock_changes_apply_to_the_stock_s3.rs` L141-161

## Cluster 47 — top score 0.9461 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/tests/every_stage_that_moves_a_number_is_on_the_card_g_visible.rs`
- `crates/rs_cam_viz/tests/the_card_names_every_entry_rule_g10.rs`

Strongest pairs:

- **0.9461** `crates/rs_cam_viz/tests/every_stage_that_moves_a_number_is_on_the_card_g_visible.rs` L479-507 ↔ `crates/rs_cam_viz/tests/the_card_names_every_entry_rule_g10.rs` L216-244

## Cluster 48 — top score 0.9457 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/compute/catalog/capabilities.rs`
- `crates/rs_cam_core/src/compute/execute/territory.rs`

Strongest pairs:

- **0.9457** `crates/rs_cam_core/src/compute/execute/territory.rs` L77-97 ↔ `crates/rs_cam_core/src/compute/catalog/capabilities.rs` L283-306

## Cluster 49 — top score 0.9456 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs`
- `crates/rs_cam_core/tests/suggest_feed_matches_final_geometry.rs`

Strongest pairs:

- **0.9456** `crates/rs_cam_core/tests/suggest_feed_matches_final_geometry.rs` L89-117 ↔ `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs` L134-168

## Cluster 50 — top score 0.9449 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/adaptive3d_entry_coalescing_f038.rs`
- `crates/rs_cam_core/tests/adaptive3d_keep_down_link_f038b.rs`

Strongest pairs:

- **0.9449** `crates/rs_cam_core/tests/adaptive3d_entry_coalescing_f038.rs` L186-209 ↔ `crates/rs_cam_core/tests/adaptive3d_keep_down_link_f038b.rs` L196-217

## Cluster 51 — top score 0.9436 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/plunge_guard_bandless_p3.rs`
- `crates/rs_cam_core/tests/plunge_guard_p3.rs`

Strongest pairs:

- **0.9436** `crates/rs_cam_core/tests/plunge_guard_bandless_p3.rs` L100-119 ↔ `crates/rs_cam_core/tests/plunge_guard_p3.rs` L108-132
- **0.9366** `crates/rs_cam_core/tests/plunge_guard_p3.rs` L133-172 ↔ `crates/rs_cam_core/tests/plunge_guard_bandless_p3.rs` L120-136

## Cluster 52 — top score 0.9424 (3 files, 3 pairs)

Files:
- `crates/rs_cam_core/tests/a_scallop_iso_field_link_clears_standing_stock_d1.rs`
- `crates/rs_cam_core/tests/every_op_conforms_w4.rs`
- `crates/rs_cam_core/tests/steep_shallow_cuts_only_the_rest_w6_4_6.rs`

Strongest pairs:

- **0.9424** `crates/rs_cam_core/tests/a_scallop_iso_field_link_clears_standing_stock_d1.rs` L101-130 ↔ `crates/rs_cam_core/tests/steep_shallow_cuts_only_the_rest_w6_4_6.rs` L218-247
- **0.9408** `crates/rs_cam_core/tests/steep_shallow_cuts_only_the_rest_w6_4_6.rs` L195-201 ↔ `crates/rs_cam_core/tests/every_op_conforms_w4.rs` L393-399
- **0.9401** `crates/rs_cam_core/tests/steep_shallow_cuts_only_the_rest_w6_4_6.rs` L327-335 ↔ `crates/rs_cam_core/tests/a_scallop_iso_field_link_clears_standing_stock_d1.rs` L193-201

## Cluster 53 — top score 0.9415 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/a_hardness_transfer_caps_at_the_printed_soft_hard_ratio_g2.rs`
- `crates/rs_cam_core/tests/every_consumer_reads_one_claimed_band_g1.rs`

Strongest pairs:

- **0.9415** `crates/rs_cam_core/tests/every_consumer_reads_one_claimed_band_g1.rs` L151-168 ↔ `crates/rs_cam_core/tests/a_hardness_transfer_caps_at_the_printed_soft_hard_ratio_g2.rs` L139-156

## Cluster 54 — top score 0.9404 (5 files, 4 pairs)

Files:
- `crates/rs_cam_core/benches/perf_suite.rs`
- `crates/rs_cam_core/src/compute/execute/standing_material.rs`
- `crates/rs_cam_core/src/finish/scallop_isofield.rs`
- `crates/rs_cam_core/tests/cavalier_shape_failure_r2.rs`
- `crates/rs_cam_core/tests/common/offset_lab.rs`

Strongest pairs:

- **0.9404** `crates/rs_cam_core/benches/perf_suite.rs` L273-297 ↔ `crates/rs_cam_core/tests/common/offset_lab.rs` L143-163
- **0.9370** `crates/rs_cam_core/tests/common/offset_lab.rs` L625-644 ↔ `crates/rs_cam_core/src/finish/scallop_isofield.rs` L183-216
- **0.9357** `crates/rs_cam_core/tests/common/offset_lab.rs` L785-807 ↔ `crates/rs_cam_core/tests/cavalier_shape_failure_r2.rs` L88-110
- **0.9335** `crates/rs_cam_core/src/compute/execute/standing_material.rs` L762-773 ↔ `crates/rs_cam_core/tests/common/offset_lab.rs` L645-656

## Cluster 55 — top score 0.9397 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/session/generation_plan.rs`
- `crates/rs_cam_viz/src/controller/generate_all.rs`

Strongest pairs:

- **0.9397** `crates/rs_cam_core/src/session/generation_plan.rs` L118-139 ↔ `crates/rs_cam_viz/src/controller/generate_all.rs` L136-153

## Cluster 56 — top score 0.9391 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/session/cycle_time.rs`
- `crates/rs_cam_viz/src/ui/readiness.rs`

Strongest pairs:

- **0.9391** `crates/rs_cam_viz/src/ui/readiness.rs` L476-492 ↔ `crates/rs_cam_core/src/session/cycle_time.rs` L220-235

## Cluster 57 — top score 0.9385 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/common/session.rs`
- `crates/rs_cam_core/tests/perf_golden_sim_metrics.rs`

Strongest pairs:

- **0.9385** `crates/rs_cam_core/tests/perf_golden_sim_metrics.rs` L467-497 ↔ `crates/rs_cam_core/tests/common/session.rs` L235-272

## Cluster 58 — top score 0.9380 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/a_vbit_row_is_read_at_its_printed_key_b4.rs`
- `crates/rs_cam_core/tests/the_onsrud_vbit_rows_serve_mdf_and_plywood_g2.rs`

Strongest pairs:

- **0.9380** `crates/rs_cam_core/tests/a_vbit_row_is_read_at_its_printed_key_b4.rs` L418-459 ↔ `crates/rs_cam_core/tests/the_onsrud_vbit_rows_serve_mdf_and_plywood_g2.rs` L289-321

## Cluster 59 — top score 0.9373 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/tests/panel_drafts_leave_egui_memory_ui09.rs`
- `crates/rs_cam_viz/tests/the_help_key_is_the_param_name_ui04.rs`

Strongest pairs:

- **0.9373** `crates/rs_cam_viz/tests/the_help_key_is_the_param_name_ui04.rs` L66-76 ↔ `crates/rs_cam_viz/tests/panel_drafts_leave_egui_memory_ui09.rs` L101-111

## Cluster 60 — top score 0.9367 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/axial_doc_step_multiple_h4.rs`
- `crates/rs_cam_core/tests/material_per_segment_s1.rs`

Strongest pairs:

- **0.9367** `crates/rs_cam_core/tests/axial_doc_step_multiple_h4.rs` L119-140 ↔ `crates/rs_cam_core/tests/material_per_segment_s1.rs` L89-110

## Cluster 61 — top score 0.9355 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/render/mesh_render.rs`
- `crates/rs_cam_viz/src/render/sim_render.rs`

Strongest pairs:

- **0.9355** `crates/rs_cam_viz/src/render/sim_render.rs` L15-40 ↔ `crates/rs_cam_viz/src/render/mesh_render.rs` L15-35

## Cluster 62 — top score 0.9349 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/adaptive3d_plans_every_slice_piece_g_adapt3dpieces.rs`
- `crates/rs_cam_core/tests/adaptive_plans_every_inset_piece_g_adaptpieces.rs`

Strongest pairs:

- **0.9349** `crates/rs_cam_core/tests/adaptive3d_plans_every_slice_piece_g_adapt3dpieces.rs` L163-175 ↔ `crates/rs_cam_core/tests/adaptive_plans_every_inset_piece_g_adaptpieces.rs` L126-138

## Cluster 63 — top score 0.9338 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/coverage_routing_pr5.rs`
- `crates/rs_cam_core/tests/generic_rest_routing_pr7.rs`

Strongest pairs:

- **0.9338** `crates/rs_cam_core/tests/coverage_routing_pr5.rs` L34-96 ↔ `crates/rs_cam_core/tests/generic_rest_routing_pr7.rs` L70-137

## Cluster 64 — top score 0.9324 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/move_to_setup_honours_index_g_dropindex.rs`
- `crates/rs_cam_core/tests/reorder_toolpath_inserts_g_dropindex.rs`

Strongest pairs:

- **0.9324** `crates/rs_cam_core/tests/move_to_setup_honours_index_g_dropindex.rs` L32-54 ↔ `crates/rs_cam_core/tests/reorder_toolpath_inserts_g_dropindex.rs` L35-57

## Cluster 65 — top score 0.9319 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/a_feed_lift_caps_at_the_cutting_ceiling_g_t18.rs`
- `crates/rs_cam_core/tests/common/mod.rs`

Strongest pairs:

- **0.9319** `crates/rs_cam_core/tests/common/mod.rs` L91-103 ↔ `crates/rs_cam_core/tests/a_feed_lift_caps_at_the_cutting_ceiling_g_t18.rs` L95-106

## Cluster 66 — top score 0.9318 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/tier_map_cache_t3.rs`
- `crates/rs_cam_core/tests/tier_map_walk_t1.rs`

Strongest pairs:

- **0.9318** `crates/rs_cam_core/tests/tier_map_walk_t1.rs` L70-82 ↔ `crates/rs_cam_core/tests/tier_map_cache_t3.rs` L41-53

## Cluster 67 — top score 0.9318 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/tests/ribbon_and_mcp_diagnostic_ids_n4.rs`
- `crates/rs_cam_viz/tests/the_inspector_text_is_bounded_ic1.rs`

Strongest pairs:

- **0.9318** `crates/rs_cam_viz/tests/ribbon_and_mcp_diagnostic_ids_n4.rs` L89-104 ↔ `crates/rs_cam_viz/tests/the_inspector_text_is_bounded_ic1.rs` L65-81

## Cluster 68 — top score 0.9313 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/dependency_edges_are_the_walker_dep1.rs`
- `crates/rs_cam_core/tests/generation_plan_is_the_edge_walk_w0b.rs`

Strongest pairs:

- **0.9313** `crates/rs_cam_core/tests/generation_plan_is_the_edge_walk_w0b.rs` L78-91 ↔ `crates/rs_cam_core/tests/dependency_edges_are_the_walker_dep1.rs` L84-97

