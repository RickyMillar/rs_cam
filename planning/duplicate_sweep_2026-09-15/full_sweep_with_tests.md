# Semantic duplicate sweep — rs_cam

Threshold: cosine >= 0.93 · chunks: 12017 (rust, >= 240 chars) · clusters: 56

Near-duplicate candidates across different files. Semantic similarity, not proof: shared boilerplate, trait impls with the same shape, and intentional parallels all appear here.

---

## Cluster 1 — top score 0.9970 (15 files, 175 pairs)

Files:
- `crates/rs_cam_core/src/metrology/monge.rs`
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

- **0.9970** `crates/rs_cam_core/tests/graded_raster_e1.rs` L219-248 ↔ `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs` L208-237
- **0.9968** `crates/rs_cam_core/tests/graded_raster_e1.rs` L441-492 ↔ `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs` L422-473
- **0.9967** `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs` L388-397 ↔ `crates/rs_cam_core/tests/graded_raster_e1.rs` L407-416
- **0.9963** `crates/rs_cam_core/tests/graded_raster_e1.rs` L585-623 ↔ `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs` L613-651
- **0.9962** `crates/rs_cam_core/tests/graded_raster_e1.rs` L417-440 ↔ `crates/rs_cam_core/tests/graded_raster_tier0_e2.rs` L398-421
- … 170 more pairs

## Cluster 2 — top score 0.9899 (10 files, 32 pairs)

Files:
- `crates/rs_cam_core/tests/hatches_are_crate_private_wp7.rs`
- `crates/rs_cam_core/tests/setters_are_crate_private_wp15b.rs`
- `crates/rs_cam_core/tests/setters_have_rows_wp15a.rs`
- `crates/rs_cam_core/tests/stale_set_has_one_answer_wp28.rs`
- `crates/rs_cam_viz/tests/effects_are_stamped_wp19.rs`
- `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs`
- `crates/rs_cam_viz/tests/non_egui_sites_write_through_commands_wp6b.rs`
- `crates/rs_cam_viz/tests/panels_read_the_token_module_up1.rs`
- `crates/rs_cam_viz/tests/production_writes_go_through_apply_wp15a.rs`
- `crates/rs_cam_viz/tests/ui_string_hygiene.rs`

Strongest pairs:

- **0.9899** `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs` L96-112 ↔ `crates/rs_cam_viz/tests/non_egui_sites_write_through_commands_wp6b.rs` L129-145
- **0.9896** `crates/rs_cam_viz/tests/non_egui_sites_write_through_commands_wp6b.rs` L146-156 ↔ `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs` L113-123
- **0.9857** `crates/rs_cam_viz/tests/production_writes_go_through_apply_wp15a.rs` L145-165 ↔ `crates/rs_cam_viz/tests/effects_are_stamped_wp19.rs` L184-204
- **0.9768** `crates/rs_cam_viz/tests/production_writes_go_through_apply_wp15a.rs` L319-357 ↔ `crates/rs_cam_viz/tests/effects_are_stamped_wp19.rs` L312-346
- **0.9741** `crates/rs_cam_viz/tests/egui_draw_sites_write_through_commands_wp6.rs` L124-161 ↔ `crates/rs_cam_viz/tests/non_egui_sites_write_through_commands_wp6b.rs` L157-200
- … 27 more pairs

## Cluster 3 — top score 0.9889 (11 files, 39 pairs)

Files:
- `crates/rs_cam_core/src/compute/execute/unified_finish_ring_collapse_g_unifiedcrash.rs`
- `crates/rs_cam_core/src/compute/execute/unified_finish_semantic_regions.rs`
- `crates/rs_cam_core/tests/classification_columns_ab_m3.rs`
- `crates/rs_cam_core/tests/common/meshes.rs`
- `crates/rs_cam_core/tests/common/tools.rs`
- `crates/rs_cam_core/tests/crease_own_region_pr6b.rs`
- `crates/rs_cam_core/tests/derived_stepover_pr6a.rs`
- `crates/rs_cam_core/tests/inert_claims_dial_f4.rs`
- `crates/rs_cam_core/tests/strategy_comparison_h4.rs`
- `crates/rs_cam_core/tests/unified_finish_dropped_band_finding_d1.rs`
- `crates/rs_cam_core/tests/unified_finish_tapered_end_to_end_m21.rs`

Strongest pairs:

- **0.9889** `crates/rs_cam_core/tests/crease_own_region_pr6b.rs` L117-135 ↔ `crates/rs_cam_core/tests/derived_stepover_pr6a.rs` L114-132
- **0.9805** `crates/rs_cam_core/tests/derived_stepover_pr6a.rs` L96-113 ↔ `crates/rs_cam_core/tests/crease_own_region_pr6b.rs` L99-116
- **0.9776** `crates/rs_cam_core/tests/crease_own_region_pr6b.rs` L83-91 ↔ `crates/rs_cam_core/tests/derived_stepover_pr6a.rs` L80-88
- **0.9767** `crates/rs_cam_core/tests/strategy_comparison_h4.rs` L327-366 ↔ `crates/rs_cam_core/tests/classification_columns_ab_m3.rs` L149-190
- **0.9736** `crates/rs_cam_core/tests/crease_own_region_pr6b.rs` L212-229 ↔ `crates/rs_cam_core/tests/derived_stepover_pr6a.rs` L209-226
- … 34 more pairs

## Cluster 4 — top score 0.9875 (2 files, 4 pairs)

Files:
- `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs`
- `crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs`

Strongest pairs:

- **0.9875** `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs` L73-90 ↔ `crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs` L64-81
- **0.9850** `crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs` L82-98 ↔ `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs` L91-107
- **0.9829** `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs` L108-131 ↔ `crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs` L99-122
- **0.9648** `crates/rs_cam_viz/tests/one_depth_caution_g_depthstockgui.rs` L131-149 ↔ `crates/rs_cam_viz/tests/depth_beyond_stock_cautions_g_depthstock.rs` L140-156

## Cluster 5 — top score 0.9860 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/tests/inspector_width_is_tab_independent_up4.rs`
- `crates/rs_cam_viz/tests/the_inspector_nests_once_dc5.rs`

Strongest pairs:

- **0.9860** `crates/rs_cam_viz/tests/inspector_width_is_tab_independent_up4.rs` L145-162 ↔ `crates/rs_cam_viz/tests/the_inspector_nests_once_dc5.rs` L163-181

## Cluster 6 — top score 0.9843 (4 files, 6 pairs)

Files:
- `crates/rs_cam_core/tests/air_filter_tool_aware_s3.rs`
- `crates/rs_cam_core/tests/entry_descent_profile_b2.rs`
- `crates/rs_cam_core/tests/profile_link_ceiling.rs`
- `crates/rs_cam_core/tests/rapid_live_check_crest_s2.rs`

Strongest pairs:

- **0.9843** `crates/rs_cam_core/tests/entry_descent_profile_b2.rs` L167-175 ↔ `crates/rs_cam_core/tests/profile_link_ceiling.rs` L153-161
- **0.9842** `crates/rs_cam_core/tests/entry_descent_profile_b2.rs` L135-153 ↔ `crates/rs_cam_core/tests/profile_link_ceiling.rs` L117-135
- **0.9416** `crates/rs_cam_core/tests/profile_link_ceiling.rs` L136-152 ↔ `crates/rs_cam_core/tests/entry_descent_profile_b2.rs` L154-166
- **0.9414** `crates/rs_cam_core/tests/profile_link_ceiling.rs` L95-104 ↔ `crates/rs_cam_core/tests/entry_descent_profile_b2.rs` L121-134
- **0.9363** `crates/rs_cam_core/tests/rapid_live_check_crest_s2.rs` L94-130 ↔ `crates/rs_cam_core/tests/air_filter_tool_aware_s3.rs` L328-364
- … 1 more pairs

## Cluster 7 — top score 0.9828 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/optimize_reports_progress_wp29.rs`
- `crates/rs_cam_core/tests/optimize_toolpath_is_a_job_wp14b.rs`

Strongest pairs:

- **0.9828** `crates/rs_cam_core/tests/optimize_toolpath_is_a_job_wp14b.rs` L246-259 ↔ `crates/rs_cam_core/tests/optimize_reports_progress_wp29.rs` L154-167
- **0.9305** `crates/rs_cam_core/tests/optimize_toolpath_is_a_job_wp14b.rs` L146-159 ↔ `crates/rs_cam_core/tests/optimize_reports_progress_wp29.rs` L122-135

## Cluster 8 — top score 0.9828 (4 files, 14 pairs)

Files:
- `crates/rs_cam_core/src/io.rs`
- `crates/rs_cam_core/src/session/project_file.rs`
- `crates/rs_cam_viz/src/controller/io.rs`
- `crates/rs_cam_viz/src/io/project.rs`

Strongest pairs:

- **0.9828** `crates/rs_cam_core/src/session/project_file.rs` L556-567 ↔ `crates/rs_cam_viz/src/io/project.rs` L1309-1320
- **0.9741** `crates/rs_cam_viz/src/io/project.rs` L336-361 ↔ `crates/rs_cam_core/src/session/project_file.rs` L321-347
- **0.9688** `crates/rs_cam_viz/src/io/project.rs` L151-201 ↔ `crates/rs_cam_core/src/session/project_file.rs` L174-220
- **0.9643** `crates/rs_cam_core/src/session/project_file.rs` L556-567 ↔ `crates/rs_cam_core/src/io.rs` L132-145
- **0.9624** `crates/rs_cam_core/src/session/project_file.rs` L270-284 ↔ `crates/rs_cam_viz/src/io/project.rs` L202-215
- … 9 more pairs

## Cluster 9 — top score 0.9815 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/adaptive/path.rs`
- `crates/rs_cam_core/src/adaptive3d/path.rs`

Strongest pairs:

- **0.9815** `crates/rs_cam_core/src/adaptive3d/path.rs` L1653-1661 ↔ `crates/rs_cam_core/src/adaptive/path.rs` L1603-1611

## Cluster 10 — top score 0.9796 (5 files, 6 pairs)

Files:
- `crates/rs_cam_core/tests/feed_optimization_refusals_wp18.rs`
- `crates/rs_cam_core/tests/gen_inputs_one_assembly_n12.rs`
- `crates/rs_cam_core/tests/job_three_steps_equal_generate_toolpath.rs`
- `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs`
- `crates/rs_cam_core/tests/resolved_gen_inputs_has_one_producer.rs`

Strongest pairs:

- **0.9796** `crates/rs_cam_core/tests/gen_inputs_one_assembly_n12.rs` L106-122 ↔ `crates/rs_cam_core/tests/feed_optimization_refusals_wp18.rs` L246-263
- **0.9659** `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs` L92-106 ↔ `crates/rs_cam_core/tests/gen_inputs_one_assembly_n12.rs` L228-242
- **0.9626** `crates/rs_cam_core/tests/resolved_gen_inputs_has_one_producer.rs` L72-86 ↔ `crates/rs_cam_core/tests/gen_inputs_one_assembly_n12.rs` L228-242
- **0.9562** `crates/rs_cam_core/tests/loose_executor_is_crate_private_wp12.rs` L92-106 ↔ `crates/rs_cam_core/tests/resolved_gen_inputs_has_one_producer.rs` L72-86
- **0.9449** `crates/rs_cam_core/tests/job_three_steps_equal_generate_toolpath.rs` L136-158 ↔ `crates/rs_cam_core/tests/gen_inputs_one_assembly_n12.rs` L123-145
- … 1 more pairs

## Cluster 11 — top score 0.9757 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/app/export.rs`
- `crates/rs_cam_viz/src/ui/export_wizard.rs`

Strongest pairs:

- **0.9757** `crates/rs_cam_viz/src/app/export.rs` L259-270 ↔ `crates/rs_cam_viz/src/ui/export_wizard.rs` L303-314

## Cluster 12 — top score 0.9737 (2 files, 3 pairs)

Files:
- `crates/rs_cam_core/tests/band_stamping_determinism_s3.rs`
- `crates/rs_cam_core/tests/swept_stamping_s1.rs`

Strongest pairs:

- **0.9737** `crates/rs_cam_core/tests/band_stamping_determinism_s3.rs` L110-136 ↔ `crates/rs_cam_core/tests/swept_stamping_s1.rs` L147-173
- **0.9629** `crates/rs_cam_core/tests/band_stamping_determinism_s3.rs` L78-109 ↔ `crates/rs_cam_core/tests/swept_stamping_s1.rs` L81-112
- **0.9538** `crates/rs_cam_core/tests/band_stamping_determinism_s3.rs` L137-202 ↔ `crates/rs_cam_core/tests/swept_stamping_s1.rs` L174-228

## Cluster 13 — top score 0.9729 (2 files, 2 pairs)

Files:
- `crates/rs_cam_viz/src/controller/tests.rs`
- `crates/rs_cam_viz/src/controller/workflow_tests.rs`

Strongest pairs:

- **0.9729** `crates/rs_cam_viz/src/controller/tests.rs` L72-119 ↔ `crates/rs_cam_viz/src/controller/workflow_tests.rs` L50-89
- **0.9662** `crates/rs_cam_viz/src/controller/workflow_tests.rs` L37-49 ↔ `crates/rs_cam_viz/src/controller/tests.rs` L55-71

## Cluster 14 — top score 0.9728 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/feed_explanation_record_t1.rs`
- `crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs`

Strongest pairs:

- **0.9728** `crates/rs_cam_core/tests/feed_explanation_record_t1.rs` L93-108 ↔ `crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs` L216-231
- **0.9567** `crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs` L412-467 ↔ `crates/rs_cam_core/tests/feed_explanation_record_t1.rs` L115-167

## Cluster 15 — top score 0.9725 (2 files, 2 pairs)

Files:
- `crates/rs_cam_viz/src/ui/feeds/shared.rs`
- `crates/rs_cam_viz/src/ui/properties/mod.rs`

Strongest pairs:

- **0.9725** `crates/rs_cam_viz/src/ui/feeds/shared.rs` L91-108 ↔ `crates/rs_cam_viz/src/ui/properties/mod.rs` L2711-2730
- **0.9613** `crates/rs_cam_viz/src/ui/properties/mod.rs` L2731-2743 ↔ `crates/rs_cam_viz/src/ui/feeds/shared.rs` L80-90

## Cluster 16 — top score 0.9698 (6 files, 10 pairs)

Files:
- `crates/rs_cam_core/tests/adaptive_feed_modulation_gcode_f036a.rs`
- `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs`
- `crates/rs_cam_core/tests/air_cut_one_time_base_g_airdenom.rs`
- `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs`
- `crates/rs_cam_core/tests/predicted_feed_gates_f035.rs`
- `crates/rs_cam_core/tests/retime_respects_no_kinematics_n7.rs`

Strongest pairs:

- **0.9698** `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs` L99-118 ↔ `crates/rs_cam_core/tests/retime_respects_no_kinematics_n7.rs` L110-129
- **0.9558** `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` L248-267 ↔ `crates/rs_cam_core/tests/adaptive_feed_modulation_gcode_f036a.rs` L45-63
- **0.9533** `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` L77-87 ↔ `crates/rs_cam_core/tests/predicted_feed_gates_f035.rs` L104-115
- **0.9533** `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs` L145-181 ↔ `crates/rs_cam_core/tests/retime_respects_no_kinematics_n7.rs` L156-191
- **0.9475** `crates/rs_cam_core/tests/drill_runtime_survives_retime_n2.rs` L88-98 ↔ `crates/rs_cam_core/tests/retime_respects_no_kinematics_n7.rs` L99-109
- … 5 more pairs

## Cluster 17 — top score 0.9692 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/steep_shallow_min_segment_pr8d.rs`
- `crates/rs_cam_core/tests/waterline_shared_finish_setup_c3.rs`

Strongest pairs:

- **0.9692** `crates/rs_cam_core/tests/waterline_shared_finish_setup_c3.rs` L100-119 ↔ `crates/rs_cam_core/tests/steep_shallow_min_segment_pr8d.rs` L114-132

## Cluster 18 — top score 0.9682 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/export_datum_setup_frame.rs`
- `crates/rs_cam_core/tests/export_honors_coolant_p0d1.rs`
- `crates/rs_cam_viz/tests/export_parity_core_vs_gui_p0.rs`

Strongest pairs:

- **0.9682** `crates/rs_cam_viz/tests/export_parity_core_vs_gui_p0.rs` L397-456 ↔ `crates/rs_cam_core/tests/export_datum_setup_frame.rs` L219-276
- **0.9627** `crates/rs_cam_viz/tests/export_parity_core_vs_gui_p0.rs` L376-388 ↔ `crates/rs_cam_core/tests/export_honors_coolant_p0d1.rs` L150-162

## Cluster 19 — top score 0.9676 (2 files, 4 pairs)

Files:
- `crates/rs_cam_core/tests/adopt_simulation_stores_prior_stocks.rs`
- `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs`

Strongest pairs:

- **0.9676** `crates/rs_cam_core/tests/adopt_simulation_stores_prior_stocks.rs` L68-99 ↔ `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs` L85-115
- **0.9664** `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs` L116-138 ↔ `crates/rs_cam_core/tests/adopt_simulation_stores_prior_stocks.rs` L100-122
- **0.9604** `crates/rs_cam_core/tests/adopt_simulation_stores_prior_stocks.rs` L123-153 ↔ `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs` L139-173
- **0.9420** `crates/rs_cam_core/tests/save_keeps_the_simulation_wp17.rs` L186-197 ↔ `crates/rs_cam_core/tests/adopt_simulation_stores_prior_stocks.rs` L163-174

## Cluster 20 — top score 0.9652 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/drill_no_targets_refuses_g_drillcentroid.rs`
- `crates/rs_cam_core/tests/drill_pick_emission_frame_g_drillpick.rs`
- `crates/rs_cam_core/tests/drill_picks_resolve_to_targets_g_drillpickstale.rs`

Strongest pairs:

- **0.9652** `crates/rs_cam_core/tests/drill_no_targets_refuses_g_drillcentroid.rs` L133-147 ↔ `crates/rs_cam_core/tests/drill_picks_resolve_to_targets_g_drillpickstale.rs` L186-200
- **0.9414** `crates/rs_cam_core/tests/drill_pick_emission_frame_g_drillpick.rs` L197-224 ↔ `crates/rs_cam_core/tests/drill_no_targets_refuses_g_drillcentroid.rs` L148-173

## Cluster 21 — top score 0.9645 (3 files, 2 pairs)

Files:
- `crates/rs_cam_viz/src/app/mcp.rs`
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/src/ui/sim_op_list.rs`

Strongest pairs:

- **0.9645** `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` L972-982 ↔ `crates/rs_cam_viz/src/ui/sim_op_list.rs` L1119-1129
- **0.9548** `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` L1371-1387 ↔ `crates/rs_cam_viz/src/app/mcp.rs` L5150-5167

## Cluster 22 — top score 0.9628 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/dexel_mesh.rs`
- `crates/rs_cam_core/src/dexel_mesh_mc.rs`

Strongest pairs:

- **0.9628** `crates/rs_cam_core/src/dexel_mesh.rs` L632-647 ↔ `crates/rs_cam_core/src/dexel_mesh_mc.rs` L549-562
- **0.9417** `crates/rs_cam_core/src/dexel_mesh_mc.rs` L563-572 ↔ `crates/rs_cam_core/src/dexel_mesh.rs` L789-800

## Cluster 23 — top score 0.9607 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/checkpoint_a_valley_matrix.rs`
- `crates/rs_cam_core/tests/checkpoint_c9_sampled_reach.rs`

Strongest pairs:

- **0.9607** `crates/rs_cam_core/tests/checkpoint_c9_sampled_reach.rs` L467-477 ↔ `crates/rs_cam_core/tests/checkpoint_a_valley_matrix.rs` L860-870

## Cluster 24 — top score 0.9591 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/isoclip_entry_ramp_g_isoclipentry.rs`
- `crates/rs_cam_core/tests/isoclip_link_rapid_g_isocliprapid.rs`

Strongest pairs:

- **0.9591** `crates/rs_cam_core/tests/isoclip_link_rapid_g_isocliprapid.rs` L110-132 ↔ `crates/rs_cam_core/tests/isoclip_entry_ramp_g_isoclipentry.rs` L123-147

## Cluster 25 — top score 0.9560 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/dressup_span_invariants.rs`
- `crates/rs_cam_core/tests/transform_provenance_fingerprints.rs`

Strongest pairs:

- **0.9560** `crates/rs_cam_core/tests/dressup_span_invariants.rs` L144-176 ↔ `crates/rs_cam_core/tests/transform_provenance_fingerprints.rs` L120-155

## Cluster 26 — top score 0.9535 (3 files, 3 pairs)

Files:
- `crates/rs_cam_core/src/geom_cache.rs`
- `crates/rs_cam_core/src/reach_map_cache.rs`
- `crates/rs_cam_core/src/tier_map_cache.rs`

Strongest pairs:

- **0.9535** `crates/rs_cam_core/src/tier_map_cache.rs` L161-185 ↔ `crates/rs_cam_core/src/reach_map_cache.rs` L112-136
- **0.9459** `crates/rs_cam_core/src/tier_map_cache.rs` L257-278 ↔ `crates/rs_cam_core/src/reach_map_cache.rs` L198-219
- **0.9384** `crates/rs_cam_core/src/tier_map_cache.rs` L186-194 ↔ `crates/rs_cam_core/src/geom_cache.rs` L227-235

## Cluster 27 — top score 0.9530 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/cut_efficiency_is_closed_form_g_specenergy.rs`
- `crates/rs_cam_core/tests/efficiency_abstains_without_kc_g_specenergy.rs`

Strongest pairs:

- **0.9530** `crates/rs_cam_core/tests/efficiency_abstains_without_kc_g_specenergy.rs` L89-99 ↔ `crates/rs_cam_core/tests/cut_efficiency_is_closed_form_g_specenergy.rs` L136-146

## Cluster 28 — top score 0.9529 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/ramp_finish.rs`
- `crates/rs_cam_core/src/spiral_finish.rs`

Strongest pairs:

- **0.9529** `crates/rs_cam_core/src/ramp_finish.rs` L471-479 ↔ `crates/rs_cam_core/src/spiral_finish.rs` L118-126

## Cluster 29 — top score 0.9528 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/multitool_plan_emission_o1.rs`
- `crates/rs_cam_core/tests/multitool_preview_u1.rs`
- `crates/rs_cam_core/tests/planned_tier_regions_boundary_o2.rs`

Strongest pairs:

- **0.9528** `crates/rs_cam_core/tests/multitool_preview_u1.rs` L67-77 ↔ `crates/rs_cam_core/tests/planned_tier_regions_boundary_o2.rs` L91-102
- **0.9303** `crates/rs_cam_core/tests/multitool_preview_u1.rs` L92-105 ↔ `crates/rs_cam_core/tests/multitool_plan_emission_o1.rs` L54-67

## Cluster 30 — top score 0.9527 (2 files, 2 pairs)

Files:
- `crates/rs_cam_viz/src/ui/optimize_modal.rs`
- `crates/rs_cam_viz/src/ui/optimize_project.rs`

Strongest pairs:

- **0.9527** `crates/rs_cam_viz/src/ui/optimize_modal.rs` L1149-1163 ↔ `crates/rs_cam_viz/src/ui/optimize_project.rs` L536-548
- **0.9523** `crates/rs_cam_viz/src/ui/optimize_project.rs` L515-535 ↔ `crates/rs_cam_viz/src/ui/optimize_modal.rs` L950-972

## Cluster 31 — top score 0.9524 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/op_model_ref_static_validation_f023.rs`
- `crates/rs_cam_core/tests/op_precondition_static_validation_f015.rs`

Strongest pairs:

- **0.9524** `crates/rs_cam_core/tests/op_precondition_static_validation_f015.rs` L48-64 ↔ `crates/rs_cam_core/tests/op_model_ref_static_validation_f023.rs` L51-67

## Cluster 32 — top score 0.9521 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs`
- `crates/rs_cam_core/tests/adaptive3d_planner_stock_xy_f027.rs`
- `crates/rs_cam_core/tests/strategy_advisor_smoke.rs`

Strongest pairs:

- **0.9521** `crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs` L83-178 ↔ `crates/rs_cam_core/tests/adaptive3d_planner_stock_xy_f027.rs` L66-170
- **0.9335** `crates/rs_cam_core/tests/strategy_advisor_smoke.rs` L24-118 ↔ `crates/rs_cam_core/tests/adaptive3d_interior_cell_parity_f029.rs` L83-178

## Cluster 33 — top score 0.9518 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/shallow_raster_slope_derate.rs`
- `crates/rs_cam_core/tests/shipped_raster_spacing_b1.rs`

Strongest pairs:

- **0.9518** `crates/rs_cam_core/tests/shallow_raster_slope_derate.rs` L50-73 ↔ `crates/rs_cam_core/tests/shipped_raster_spacing_b1.rs` L223-253

## Cluster 34 — top score 0.9517 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/compute/config.rs`
- `crates/rs_cam_core/src/unified_finish.rs`

Strongest pairs:

- **0.9517** `crates/rs_cam_core/src/compute/config.rs` L1124-1182 ↔ `crates/rs_cam_core/src/unified_finish.rs` L1007-1035

## Cluster 35 — top score 0.9511 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/tests/feedopt_caps_plunges_g_feedoptplunge.rs`
- `crates/rs_cam_core/tests/feedopt_clamp_never_panics_wp21.rs`

Strongest pairs:

- **0.9511** `crates/rs_cam_core/tests/feedopt_clamp_never_panics_wp21.rs` L144-153 ↔ `crates/rs_cam_core/tests/feedopt_caps_plunges_g_feedoptplunge.rs` L436-445
- **0.9414** `crates/rs_cam_core/tests/feedopt_caps_plunges_g_feedoptplunge.rs` L96-119 ↔ `crates/rs_cam_core/tests/feedopt_clamp_never_panics_wp21.rs` L79-109

## Cluster 36 — top score 0.9510 (3 files, 4 pairs)

Files:
- `crates/rs_cam_core/tests/command_registry_completeness.rs`
- `crates/rs_cam_core/tests/mcp_mutation_rows_reach_core.rs`
- `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs`

Strongest pairs:

- **0.9510** `crates/rs_cam_core/tests/command_registry_completeness.rs` L300-333 ↔ `crates/rs_cam_core/tests/mcp_mutation_rows_reach_core.rs` L155-189
- **0.9462** `crates/rs_cam_core/tests/mcp_mutation_rows_reach_core.rs` L143-154 ↔ `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs` L191-205
- **0.9432** `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs` L191-205 ↔ `crates/rs_cam_core/tests/command_registry_completeness.rs` L285-299
- **0.9370** `crates/rs_cam_core/tests/mcp_mutation_rows_reach_core.rs` L211-226 ↔ `crates/rs_cam_core/tests/command_registry_completeness.rs` L359-378

## Cluster 37 — top score 0.9504 (7 files, 11 pairs)

Files:
- `crates/rs_cam_viz/tests/add_toolpath_via_gui_g_guiadd.rs`
- `crates/rs_cam_viz/tests/apply_contract_a3.rs`
- `crates/rs_cam_viz/tests/feeds_apply_drops_result_n13.rs`
- `crates/rs_cam_viz/tests/generate_all_fixpoint_parity.rs`
- `crates/rs_cam_viz/tests/get_notifications_g_toastread.rs`
- `crates/rs_cam_viz/tests/mcp_toasts_report_outcome_g_mcptoast.rs`
- `crates/rs_cam_viz/tests/the_speeds_apply_holds_the_cut_g_speedsonly.rs`

Strongest pairs:

- **0.9504** `crates/rs_cam_viz/tests/get_notifications_g_toastread.rs` L136-154 ↔ `crates/rs_cam_viz/tests/apply_contract_a3.rs` L56-74
- **0.9471** `crates/rs_cam_viz/tests/mcp_toasts_report_outcome_g_mcptoast.rs` L233-251 ↔ `crates/rs_cam_viz/tests/get_notifications_g_toastread.rs` L136-154
- **0.9452** `crates/rs_cam_viz/tests/apply_contract_a3.rs` L56-74 ↔ `crates/rs_cam_viz/tests/feeds_apply_drops_result_n13.rs` L74-92
- **0.9389** `crates/rs_cam_viz/tests/get_notifications_g_toastread.rs` L136-154 ↔ `crates/rs_cam_viz/tests/feeds_apply_drops_result_n13.rs` L74-92
- **0.9381** `crates/rs_cam_viz/tests/the_speeds_apply_holds_the_cut_g_speedsonly.rs` L70-88 ↔ `crates/rs_cam_viz/tests/apply_contract_a3.rs` L56-74
- … 6 more pairs

## Cluster 38 — top score 0.9499 (4 files, 5 pairs)

Files:
- `crates/rs_cam_core/src/debug_trace.rs`
- `crates/rs_cam_core/src/semantic_trace.rs`
- `crates/rs_cam_core/src/simulation_cut.rs`
- `crates/rs_cam_viz/src/state/simulation.rs`

Strongest pairs:

- **0.9499** `crates/rs_cam_core/src/debug_trace.rs` L550-568 ↔ `crates/rs_cam_core/src/semantic_trace.rs` L1225-1243
- **0.9451** `crates/rs_cam_core/src/debug_trace.rs` L124-134 ↔ `crates/rs_cam_core/src/semantic_trace.rs` L497-508
- **0.9367** `crates/rs_cam_viz/src/state/simulation.rs` L2919-3018 ↔ `crates/rs_cam_core/src/simulation_cut.rs` L1954-2053
- **0.9336** `crates/rs_cam_core/src/semantic_trace.rs` L1225-1243 ↔ `crates/rs_cam_core/src/simulation_cut.rs` L1845-1863
- **0.9329** `crates/rs_cam_core/src/semantic_trace.rs` L1204-1224 ↔ `crates/rs_cam_core/src/debug_trace.rs` L529-549

## Cluster 39 — top score 0.9493 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/tests/mcp_authoring_surface.rs`
- `crates/rs_cam_viz/tests/mcp_rebind_surface_g_mcprebind.rs`

Strongest pairs:

- **0.9493** `crates/rs_cam_viz/tests/mcp_authoring_surface.rs` L42-54 ↔ `crates/rs_cam_viz/tests/mcp_rebind_surface_g_mcprebind.rs` L37-49

## Cluster 40 — top score 0.9472 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/feeds/suggest.rs`
- `crates/rs_cam_core/tests/suggest_feed_matches_final_geometry.rs`

Strongest pairs:

- **0.9472** `crates/rs_cam_core/tests/suggest_feed_matches_final_geometry.rs` L89-111 ↔ `crates/rs_cam_core/src/feeds/suggest.rs` L2613-2643

## Cluster 41 — top score 0.9471 (4 files, 3 pairs)

Files:
- `crates/rs_cam_cli/src/project.rs`
- `crates/rs_cam_core/src/session/mod.rs`
- `crates/rs_cam_core/src/session/mutation.rs`
- `crates/rs_cam_viz/src/state/job.rs`

Strongest pairs:

- **0.9471** `crates/rs_cam_core/src/session/mod.rs` L1149-1218 ↔ `crates/rs_cam_cli/src/project.rs` L48-107
- **0.9367** `crates/rs_cam_viz/src/state/job.rs` L130-143 ↔ `crates/rs_cam_core/src/session/mod.rs` L564-588
- **0.9336** `crates/rs_cam_core/src/session/mutation.rs` L78-114 ↔ `crates/rs_cam_viz/src/state/job.rs` L434-463

## Cluster 42 — top score 0.9469 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/feeds/vendor_normalize.rs`
- `crates/rs_cam_core/src/tool_load/optimize/context.rs`

Strongest pairs:

- **0.9469** `crates/rs_cam_core/src/feeds/vendor_normalize.rs` L125-141 ↔ `crates/rs_cam_core/src/tool_load/optimize/context.rs` L152-168

## Cluster 43 — top score 0.9456 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/adaptive3d_entry_coalescing_f038.rs`
- `crates/rs_cam_core/tests/adaptive3d_keep_down_link_f038b.rs`

Strongest pairs:

- **0.9456** `crates/rs_cam_core/tests/adaptive3d_entry_coalescing_f038.rs` L181-204 ↔ `crates/rs_cam_core/tests/adaptive3d_keep_down_link_f038b.rs` L176-197

## Cluster 44 — top score 0.9434 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/compute/operation_configs.rs`
- `crates/rs_cam_core/src/project_curve.rs`

Strongest pairs:

- **0.9434** `crates/rs_cam_core/src/project_curve.rs` L28-39 ↔ `crates/rs_cam_core/src/compute/operation_configs.rs` L1583-1596

## Cluster 45 — top score 0.9426 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/ui/components/compare.rs`
- `crates/rs_cam_viz/src/ui/feeds/compare.rs`

Strongest pairs:

- **0.9426** `crates/rs_cam_viz/src/ui/components/compare.rs` L85-92 ↔ `crates/rs_cam_viz/src/ui/feeds/compare.rs` L697-709

## Cluster 46 — top score 0.9419 (4 files, 3 pairs)

Files:
- `crates/rs_cam_core/benches/perf_suite.rs`
- `crates/rs_cam_core/src/scallop_isofield.rs`
- `crates/rs_cam_core/tests/cavalier_shape_failure_r2.rs`
- `crates/rs_cam_core/tests/common/offset_lab.rs`

Strongest pairs:

- **0.9419** `crates/rs_cam_core/src/scallop_isofield.rs` L183-216 ↔ `crates/rs_cam_core/tests/common/offset_lab.rs` L625-644
- **0.9404** `crates/rs_cam_core/tests/common/offset_lab.rs` L143-163 ↔ `crates/rs_cam_core/benches/perf_suite.rs` L268-292
- **0.9357** `crates/rs_cam_core/tests/common/offset_lab.rs` L785-807 ↔ `crates/rs_cam_core/tests/cavalier_shape_failure_r2.rs` L88-110

## Cluster 47 — top score 0.9416 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/gcode/mod.rs`
- `crates/rs_cam_viz/src/io/export.rs`

Strongest pairs:

- **0.9416** `crates/rs_cam_viz/src/io/export.rs` L344-360 ↔ `crates/rs_cam_core/src/gcode/mod.rs` L801-819

## Cluster 48 — top score 0.9386 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/reach_map.rs`
- `crates/rs_cam_core/src/tier_map.rs`

Strongest pairs:

- **0.9386** `crates/rs_cam_core/src/tier_map.rs` L737-747 ↔ `crates/rs_cam_core/src/reach_map.rs` L920-930
- **0.9360** `crates/rs_cam_core/src/tier_map.rs` L784-833 ↔ `crates/rs_cam_core/src/reach_map.rs` L981-1021

## Cluster 49 — top score 0.9384 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/benches/hot_paths.rs`
- `crates/rs_cam_core/tests/pushcutter_band_query_g1.rs`

Strongest pairs:

- **0.9384** `crates/rs_cam_core/tests/pushcutter_band_query_g1.rs` L223-251 ↔ `crates/rs_cam_core/benches/hot_paths.rs` L97-126

## Cluster 50 — top score 0.9369 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/common/session.rs`
- `crates/rs_cam_core/tests/perf_golden_sim_metrics.rs`

Strongest pairs:

- **0.9369** `crates/rs_cam_core/tests/common/session.rs` L135-172 ↔ `crates/rs_cam_core/tests/perf_golden_sim_metrics.rs` L369-399

## Cluster 51 — top score 0.9355 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/render/mesh_render.rs`
- `crates/rs_cam_viz/src/render/sim_render.rs`

Strongest pairs:

- **0.9355** `crates/rs_cam_viz/src/render/sim_render.rs` L15-40 ↔ `crates/rs_cam_viz/src/render/mesh_render.rs` L15-35

## Cluster 52 — top score 0.9352 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/query_cycle_time_one_answer.rs`
- `crates/rs_cam_viz/tests/cycle_time_basis_g_timeest.rs`

Strongest pairs:

- **0.9352** `crates/rs_cam_viz/tests/cycle_time_basis_g_timeest.rs` L89-111 ↔ `crates/rs_cam_core/tests/query_cycle_time_one_answer.rs` L130-146

## Cluster 53 — top score 0.9352 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/tests/rest_badge_one_predicate_g_restbadge.rs`
- `crates/rs_cam_viz/tests/ribbon_and_mcp_diagnostic_ids_n4.rs`

Strongest pairs:

- **0.9352** `crates/rs_cam_viz/tests/rest_badge_one_predicate_g_restbadge.rs` L54-71 ↔ `crates/rs_cam_viz/tests/ribbon_and_mcp_diagnostic_ids_n4.rs` L87-104

## Cluster 54 — top score 0.9338 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/coverage_routing_pr5.rs`
- `crates/rs_cam_core/tests/generic_rest_routing_pr7.rs`

Strongest pairs:

- **0.9338** `crates/rs_cam_core/tests/generic_rest_routing_pr7.rs` L70-137 ↔ `crates/rs_cam_core/tests/coverage_routing_pr5.rs` L49-111

## Cluster 55 — top score 0.9318 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/tests/tier_map_cache_t3.rs`
- `crates/rs_cam_core/tests/tier_map_walk_t1.rs`

Strongest pairs:

- **0.9318** `crates/rs_cam_core/tests/tier_map_walk_t1.rs` L70-82 ↔ `crates/rs_cam_core/tests/tier_map_cache_t3.rs` L41-53

## Cluster 56 — top score 0.9311 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/tests/command_surface_completeness.rs`
- `crates/rs_cam_viz/tests/optimize_runs_on_the_job_lane_wp14b.rs`

Strongest pairs:

- **0.9311** `crates/rs_cam_viz/tests/command_surface_completeness.rs` L77-92 ↔ `crates/rs_cam_viz/tests/optimize_runs_on_the_job_lane_wp14b.rs` L84-98

