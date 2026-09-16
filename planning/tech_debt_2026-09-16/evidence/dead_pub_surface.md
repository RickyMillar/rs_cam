# Dead public surface (mechanical)

Definitions: 5063 pub items in production code. Word-count instrument over crates/*/{src,tests,benches,examples}; a name that occurs once in the whole workspace is its own definition and nothing else. Trait-impl methods and macro-generated uses are blind spots: VERIFY each row with rg before acting.

## Zero references anywhere (65)

| kind | name | file:line | vis |
|---|---|---|---|
| fn | `adaptive_toolpath_annotated_traced_with_cancel` | `crates/rs_cam_core/src/adaptive/mod.rs:280` | pub |
| fn | `ui_style` | `crates/rs_cam_core/src/compute/catalog.rs:913` | pub |
| const | `ALL_SIMPLE` | `crates/rs_cam_core/src/compute/config.rs:1765` | pub |
| fn | `incr_counter` | `crates/rs_cam_core/src/debug_trace.rs:440` | pub |
| fn | `realised_step_down` | `crates/rs_cam_core/src/depth.rs:68` | pub |
| fn | `ray_is_empty` | `crates/rs_cam_core/src/dexel.rs:198` | pub |
| fn | `is_headline` | `crates/rs_cam_core/src/diagnostics/mod.rs:297` | pub |
| fn | `edges_for_face` | `crates/rs_cam_core/src/enriched_mesh.rs:164` | pub |
| fn | `edges_between` | `crates/rs_cam_core/src/enriched_mesh.rs:272` | pub |
| fn | `edge_chains_2d` | `crates/rs_cam_core/src/enriched_mesh.rs:281` | pub |
| fn | `load_dir` | `crates/rs_cam_core/src/feeds/vendor_lut.rs:398` | pub |
| fn | `overlaps_xy` | `crates/rs_cam_core/src/geo.rs:69` | pub |
| fn | `is_built` | `crates/rs_cam_core/src/geom_cache.rs:310` | pub |
| fn | `at_index_mut` | `crates/rs_cam_core/src/grid2.rs:171` | pub |
| fn | `as_mut_slice` | `crates/rs_cam_core/src/grid2.rs:199` | pub |
| fn | `path_for` | `crates/rs_cam_core/src/machine_library.rs:88` | pub |
| fn | `from_labels` | `crates/rs_cam_core/src/metrology/ownership.rs:74` | pub |
| fn | `pencil_toolpath_annotated` | `crates/rs_cam_core/src/pencil.rs:2494` | pub |
| fn | `ramp_finish_toolpath_with_cancel` | `crates/rs_cam_core/src/ramp_finish.rs:459` | pub |
| fn | `ramp_finish_toolpath_annotated` | `crates/rs_cam_core/src/ramp_finish.rs:880` | pub |
| fn | `scallop_toolpath_annotated` | `crates/rs_cam_core/src/scallop.rs:2806` | pub |
| fn | `set_param_json` | `crates/rs_cam_core/src/semantic_trace.rs:873` | pub |
| fn | `push_move` | `crates/rs_cam_core/src/semantic_trace.rs:956` | pub |
| fn | `bind_scope_to_current_range` | `crates/rs_cam_core/src/semantic_trace.rs:964` | pub |
| fn | `item_ids_covering_move` | `crates/rs_cam_core/src/semantic_trace.rs:975` | pub |
| fn | `export_diagnostics_json` | `crates/rs_cam_core/src/session/compute.rs:5787` | pub |
| fn | `all_toolpath_ids` | `crates/rs_cam_core/src/session/mod.rs:1879` | pub |
| fn | `publishable` | `crates/rs_cam_core/src/sim_measurability.rs:259` | pub |
| fn | `engagement_is_floor_prone` | `crates/rs_cam_core/src/sim_measurability.rs:465` | pub |
| fn | `spiral_finish_toolpath_with_cancel` | `crates/rs_cam_core/src/spiral_finish.rs:106` | pub |
| fn | `spiral_finish_toolpath_annotated` | `crates/rs_cam_core/src/spiral_finish.rs:310` | pub |
| fn | `coarseness_clamped` | `crates/rs_cam_core/src/tier_islands.rs:338` | pub |
| fn | `new_from_below` | `crates/rs_cam_core/src/tool/mod.rs:57` | pub |
| fn | `save_library` | `crates/rs_cam_core/src/tool_library.rs:152` | pub |
| fn | `append_tool` | `crates/rs_cam_core/src/tool_library.rs:171` | pub |
| fn | `all_tools` | `crates/rs_cam_core/src/tool_library.rs:214` | pub |
| fn | `evaluate_project` | `crates/rs_cam_core/src/tool_load/mod.rs:567` | pub |
| fn | `active_axes` | `crates/rs_cam_core/src/tool_load/optimize/axes.rs:228` | pub |
| fn | `toolpath_to_3d_html` | `crates/rs_cam_core/src/viz.rs:373` | pub |
| fn | `simulation_3d_html` | `crates/rs_cam_core/src/viz.rs:694` | pub |
| fn | `simulation_viewport_and_events_mut` | `crates/rs_cam_viz/src/controller.rs:268` | pub |
| fn | `list_presets` | `crates/rs_cam_viz/src/io/presets.rs:27` | pub |
| fn | `save_preset` | `crates/rs_cam_viz/src/io/presets.rs:47` | pub |
| const | `ENTRY_PREVIEW` | `crates/rs_cam_viz/src/render/colors.rs:27` | pub |
| const | `STOCK_DEFAULT` | `crates/rs_cam_viz/src/render/colors.rs:38` | pub |
| const | `DEVIATION_ON_TARGET` | `crates/rs_cam_viz/src/render/colors.rs:41` | pub |
| const | `COLLISION_POINT` | `crates/rs_cam_viz/src/render/colors.rs:50` | pub |
| const | `MESH_HIGHLIGHT` | `crates/rs_cam_viz/src/render/colors.rs:78` | pub |
| const | `MESH_HOVER` | `crates/rs_cam_viz/src/render/colors.rs:80` | pub |
| fn | `transform_heightmap_mesh` | `crates/rs_cam_viz/src/state/job.rs:318` | pub |
| fn | `clear_runtime` | `crates/rs_cam_viz/src/state/runtime.rs:59` | pub |
| struct | `ToolpathView` | `crates/rs_cam_viz/src/state/runtime.rs:85` | pub |
| fn | `current_boundary_index` | `crates/rs_cam_viz/src/state/simulation.rs:1252` | pub |
| fn | `trace_availability_for_toolpath` | `crates/rs_cam_viz/src/state/simulation.rs:1324` | pub |
| fn | `toolpath_cut_summary` | `crates/rs_cam_viz/src/state/simulation.rs:1389` | pub |
| fn | `semantic_cut_summary` | `crates/rs_cam_viz/src/state/simulation.rs:1402` | pub |
| fn | `cut_worst_items` | `crates/rs_cam_viz/src/state/simulation.rs:1418` | pub |
| fn | `cut_hotspots` | `crates/rs_cam_viz/src/state/simulation.rs:1459` | pub |
| fn | `trace_target_for_annotated_span` | `crates/rs_cam_viz/src/state/simulation.rs:1734` | pub |
| fn | `current_item_bbox` | `crates/rs_cam_viz/src/state/simulation.rs:1837` | pub |
| fn | `all_visible` | `crates/rs_cam_viz/src/state/viewport.rs:34` | pub |
| fn | `row_hover_tint` | `crates/rs_cam_viz/src/ui/components/kv_row.rs:250` | pub |
| fn | `mcp_highlight_effect` | `crates/rs_cam_viz/src/ui/properties/mod.rs:73` | pub |
| fn | `json_f64` | `crates/rs_cam_viz/src/ui/sim_debug.rs:153` | pub |
| const | `SPACE_0` | `crates/rs_cam_viz/src/ui/tokens.rs:36` | pub |

## `pub` but referenced only inside its own file (208) — visibility too wide, or dead behind a same-file caller

| kind | name | file:line |
|---|---|---|
| struct | `SetupDef` | `crates/rs_cam_cli/src/job.rs:110` |
| struct | `JobConfig` | `crates/rs_cam_cli/src/job.rs:118` |
| struct | `OpResult` | `crates/rs_cam_cli/src/job.rs:391` |
| struct | `DiffOutcome` | `crates/rs_cam_cli/src/smoke.rs:187` |
| fn | `get_at` | `crates/rs_cam_core/src/adaptive/material_grid.rs:111` |
| fn | `adaptive_toolpath_with_cancel` | `crates/rs_cam_core/src/adaptive/mod.rs:236` |
| fn | `adaptive_toolpath_traced_with_cancel` | `crates/rs_cam_core/src/adaptive/mod.rs:244` |
| fn | `adaptive_3d_toolpath_traced_with_cancel` | `crates/rs_cam_core/src/adaptive3d/mod.rs:433` |
| fn | `adaptive_3d_toolpath_annotated_with_cancel` | `crates/rs_cam_core/src/adaptive3d/mod.rs:463` |
| const | `CANCEL_WINDOW_CELLS` | `crates/rs_cam_core/src/classify_probe.rs:69` |
| const | `CANCEL_WINDOW_FACES` | `crates/rs_cam_core/src/classify_probe.rs:78` |
| fn | `classification_probe` | `crates/rs_cam_core/src/classify_probe.rs:269` |
| fn | `segments_from_profile` | `crates/rs_cam_core/src/collision.rs:91` |
| fn | `check_collisions_interpolated` | `crates/rs_cam_core/src/collision.rs:183` |
| const | `RAPID_CLEARANCE_TOLERANCE_CELLS` | `crates/rs_cam_core/src/collision.rs:572` |
| fn | `rapid_clearance_tolerance_mm` | `crates/rs_cam_core/src/collision.rs:576` |
| struct | `ParamHint` | `crates/rs_cam_core/src/compute/catalog.rs:1197` |
| struct | `OperationParamSchema` | `crates/rs_cam_core/src/compute/catalog.rs:1207` |
| fn | `to_json` | `crates/rs_cam_core/src/compute/catalog.rs:1333` |
| const | `PREFER_HELIX` | `crates/rs_cam_core/src/compute/catalog.rs:1535` |
| fn | `new_default_with_ctx` | `crates/rs_cam_core/src/compute/catalog.rs:2691` |
| fn | `untouched_material` | `crates/rs_cam_core/src/compute/config.rs:1320` |
| fn | `reached_uncut_estimate` | `crates/rs_cam_core/src/compute/config.rs:1340` |
| const | `STALE_DRILL_PICKS_PHRASE` | `crates/rs_cam_core/src/compute/execute.rs:1005` |
| const | `DEFAULT_MAX_BYTES` | `crates/rs_cam_core/src/compute/sim_prefix.rs:207` |
| const | `CARBIDE_YOUNGS_MODULUS_N_PER_MM2` | `crates/rs_cam_core/src/compute/tool_config.rs:111` |
| const | `HSS_YOUNGS_MODULUS_N_PER_MM2` | `crates/rs_cam_core/src/compute/tool_config.rs:114` |
| fn | `cross_type_geometry` | `crates/rs_cam_core/src/compute/tool_config.rs:289` |
| fn | `defines_owner` | `crates/rs_cam_core/src/compute/tool_config.rs:578` |
| fn | `is_length` | `crates/rs_cam_core/src/compute/tool_config.rs:583` |
| const | `PAPER_NEAR_CENTRE_RADIUS` | `crates/rs_cam_core/src/conformal_spiral.rs:268` |
| const | `PAPER_NEAR_CENTRE_BRIDGE_SHIFT` | `crates/rs_cam_core/src/conformal_spiral.rs:271` |
| const | `PAPER_SHIFT_STEP` | `crates/rs_cam_core/src/conformal_spiral.rs:288` |
| fn | `roughing_pass_count` | `crates/rs_cam_core/src/depth.rs:101` |
| fn | `depth_stepped_with_finish` | `crates/rs_cam_core/src/depth.rs:248` |
| fn | `clear_below_at` | `crates/rs_cam_core/src/dexel_stock/mod.rs:582` |
| fn | `simulate_toolpath_with_cancel` | `crates/rs_cam_core/src/dexel_stock/simulation.rs:48` |
| fn | `simulate_toolpath_range_with_lut` | `crates/rs_cam_core/src/dexel_stock/simulation.rs:837` |
| const | `PROJECT_KINEMATIC_UTILIZATION` | `crates/rs_cam_core/src/diagnostics/ids.rs:84` |
| struct | `ReferenceEngagement` | `crates/rs_cam_core/src/dressup.rs:2702` |
| struct | `OutsideRegionChord` | `crates/rs_cam_core/src/entry_audit.rs:139` |
| fn | `mid_mm_per_tooth` | `crates/rs_cam_core/src/feed_modulation.rs:145` |
| fn | `from_machine` | `crates/rs_cam_core/src/feeds/explain_payload.rs:36` |
| struct | `FormulaBreakdown` | `crates/rs_cam_core/src/feeds/mod.rs:619` |
| const | `POWER_LADDER_AP_FLOOR_MM` | `crates/rs_cam_core/src/feeds/mod.rs:1154` |
| const | `POWER_LADDER_AE_FLOOR_MM` | `crates/rs_cam_core/src/feeds/mod.rs:1158` |
| const | `COLLET_EXPOSURE_MARGIN_MM` | `crates/rs_cam_core/src/feeds/predict.rs:79` |
| struct | `Predictions` | `crates/rs_cam_core/src/feeds/profile.rs:47` |
| struct | `ConstraintEnvelopes` | `crates/rs_cam_core/src/feeds/profile.rs:66` |
| fn | `edge_radius_floor` | `crates/rs_cam_core/src/feeds/provenance.rs:76` |
| fn | `from_chipload_source` | `crates/rs_cam_core/src/feeds/provenance.rs:95` |
| fn | `render_label` | `crates/rs_cam_core/src/feeds/quantities.rs:275` |
| const | `LEGACY_ESTIMATE_NOTE` | `crates/rs_cam_core/src/feeds/rationale.rs:56` |
| const | `MIN_CHIPLOAD_RANGE_FRACTION` | `crates/rs_cam_core/src/feeds/vendor_lut.rs:441` |
| fn | `detect_conflicting_rows` | `crates/rs_cam_core/src/feeds/vendor_lut.rs:551` |
| fn | `render_toolpath_composite_in_frame` | `crates/rs_cam_core/src/fingerprint.rs:766` |
| fn | `save_mesh_composite_png` | `crates/rs_cam_core/src/fingerprint.rs:843` |
| struct | `FinishSurfaceCacheStats` | `crates/rs_cam_core/src/finish_surface_cache.rs:261` |
| enum | `SimEvidenceMeta` | `crates/rs_cam_core/src/gcode/mod.rs:487` |
| fn | `effective_trace` | `crates/rs_cam_core/src/gcode/mod.rs:514` |
| struct | `GeomCacheStats` | `crates/rs_cam_core/src/geom_cache.rs:163` |
| fn | `rc_of` | `crates/rs_cam_core/src/grid2.rs:128` |
| fn | `at_index` | `crates/rs_cam_core/src/grid2.rs:167` |
| fn | `iter_rc` | `crates/rs_cam_core/src/grid2.rs:188` |
| fn | `into_vec` | `crates/rs_cam_core/src/grid2.rs:203` |
| struct | `GrblImport` | `crates/rs_cam_core/src/machine_kinematics.rs:172` |
| struct | `MoveKinematics` | `crates/rs_cam_core/src/machine_kinematics.rs:482` |
| const | `CHAIN_QUANTIZE_SCALE` | `crates/rs_cam_core/src/marching_squares.rs:139` |
| fn | `resolution_label` | `crates/rs_cam_core/src/measurement.rs:352` |
| struct | `WindingReport` | `crates/rs_cam_core/src/mesh.rs:21` |
| fn | `from_stl_bytes` | `crates/rs_cam_core/src/mesh.rs:140` |
| const | `PRIZE_RATIO_BANDS` | `crates/rs_cam_core/src/metrology/census.rs:82` |
| fn | `best_fixed_bound` | `crates/rs_cam_core/src/metrology/census.rs:500` |
| fn | `times_floor` | `crates/rs_cam_core/src/metrology/floor.rs:111` |
| fn | `owner_at` | `crates/rs_cam_core/src/metrology/ownership.rs:91` |
| const | `ENTRY_RAMP_BITE_TIP_FRACTION` | `crates/rs_cam_core/src/pencil.rs:1034` |
| const | `ENTRY_RAMP_MAX_BITE_MM` | `crates/rs_cam_core/src/pencil.rs:1040` |
| const | `ENTRY_RAMP_MAX_ANGLE_DEG` | `crates/rs_cam_core/src/pencil.rs:1057` |
| const | `ENTRY_RAMP_MIN_WINDOW_MM` | `crates/rs_cam_core/src/pencil.rs:1060` |
| const | `ENTRY_RAMP_MAX_LAPS` | `crates/rs_cam_core/src/pencil.rs:1066` |
| fn | `pocket_toolpath_reported_with_cancel` | `crates/rs_cam_core/src/pocket.rs:80` |
| const | `CHORD_TOLERANCE_SHARE` | `crates/rs_cam_core/src/polygon.rs:1123` |
| const | `MIN_DEVIATION_MM` | `crates/rs_cam_core/src/polygon.rs:1129` |
| fn | `from_polygons` | `crates/rs_cam_core/src/polygon.rs:1457` |
| fn | `offset_per_group_reported` | `crates/rs_cam_core/src/polygon.rs:1536` |
| const | `PUSH_QUERY_SLACK_MM` | `crates/rs_cam_core/src/pushcutter.rs:46` |
| fn | `push_cutter_fiber_over` | `crates/rs_cam_core/src/pushcutter.rs:185` |
| const | `AREA_PROVENANCE` | `crates/rs_cam_core/src/ramp_finish.rs:152` |
| fn | `implied_depth_mm` | `crates/rs_cam_core/src/reach.rs:221` |
| fn | `rim_rise_mm` | `crates/rs_cam_core/src/reach.rs:453` |
| const | `MIN_REACH_CELL_MM` | `crates/rs_cam_core/src/reach_map.rs:214` |
| const | `MAX_REACH_CELL_MM` | `crates/rs_cam_core/src/reach_map.rs:227` |
| struct | `ReachMapCacheStats` | `crates/rs_cam_core/src/reach_map_cache.rs:98` |
| fn | `point_stepover` | `crates/rs_cam_core/src/scallop.rs:716` |
| fn | `scallop_toolpath_research_with_stage` | `crates/rs_cam_core/src/scallop.rs:2173` |
| fn | `scallop_height_curved` | `crates/rs_cam_core/src/scallop_math.rs:85` |
| fn | `insert_json` | `crates/rs_cam_core/src/semantic_trace.rs:405` |
| fn | `has_clearing_strategy` | `crates/rs_cam_core/src/session/compute.rs:1798` |
| struct | `ToolpathSummary` | `crates/rs_cam_core/src/session/mod.rs:998` |
| struct | `ToolSummary` | `crates/rs_cam_core/src/session/mod.rs:1020` |
| fn | `from_simulation` | `crates/rs_cam_core/src/session/mod.rs:1345` |
| fn | `from_project_file_with_warnings` | `crates/rs_cam_core/src/session/mod.rs:1579` |
| fn | `reach_map_for` | `crates/rs_cam_core/src/session/reach.rs:195` |
| const | `NOT_MEASURABLE_BLIND_FRACTION` | `crates/rs_cam_core/src/sim_measurability.rs:288` |
| const | `DEGRADED_BLIND_FRACTION` | `crates/rs_cam_core/src/sim_measurability.rs:293` |
| const | `MIN_SPATIAL_BUCKET_MM` | `crates/rs_cam_core/src/sim_triage.rs:70` |
| fn | `worst_severity` | `crates/rs_cam_core/src/sim_triage.rs:238` |
| type | `RegionResolver` | `crates/rs_cam_core/src/sim_triage.rs:252` |
| const | `STANDING_MATERIAL_SAMPLE_FRACTION` | `crates/rs_cam_core/src/sim_triage.rs:481` |
| const | `STANDING_MATERIAL_MULTIPLE` | `crates/rs_cam_core/src/sim_triage.rs:484` |
| const | `ENTRY_LOAD_SEVERE_PEAK_MM` | `crates/rs_cam_core/src/sim_triage.rs:628` |
| const | `ENTRY_LOAD_MIN_BODY_SAMPLES` | `crates/rs_cam_core/src/sim_triage.rs:632` |
| struct | `RebasedCuttingTimes` | `crates/rs_cam_core/src/simulation_cut.rs:691` |
| fn | `from_samples_with_semantics` | `crates/rs_cam_core/src/simulation_cut.rs:1066` |
| fn | `finalize_per_kinematics` | `crates/rs_cam_core/src/simulation_cut.rs:1339` |
| struct | `CompactSpiral` | `crates/rs_cam_core/src/spiral_finish_compact.rs:256` |
| const | `SHALLOW_HALF_LABEL` | `crates/rs_cam_core/src/steep_shallow.rs:670` |
| fn | `load_svg_data_mm` | `crates/rs_cam_core/src/svg_input.rs:72` |
| fn | `effective_close_radius_mm` | `crates/rs_cam_core/src/tier_islands.rs:345` |
| fn | `effective_min_region_area_mm2` | `crates/rs_cam_core/src/tier_islands.rs:355` |
| struct | `TierMapCacheStats` | `crates/rs_cam_core/src/tier_map_cache.rs:142` |
| fn | `update_z_min` | `crates/rs_cam_core/src/tool/mod.rs:68` |
| fn | `update_tool_at_in` | `crates/rs_cam_core/src/tool_library.rs:253` |
| fn | `dedupe_in` | `crates/rs_cam_core/src/tool_library.rs:374` |
| fn | `first_span_of_kind` | `crates/rs_cam_core/src/tool_load/locality.rs:54` |
| fn | `resolve_rpm_bounds` | `crates/rs_cam_core/src/tool_load/optimize/bounds.rs:259` |
| fn | `marginal_safe` | `crates/rs_cam_core/src/tool_load/optimize/outcome.rs:498` |
| fn | `trade_off` | `crates/rs_cam_core/src/tool_load/optimize/outcome.rs:517` |
| fn | `first_marginal_safe_index` | `crates/rs_cam_core/src/tool_load/optimize/outcome.rs:649` |
| struct | `AxesPolicy` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:45` |
| struct | `FeedPolicy` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:68` |
| struct | `RetargetPolicy` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:79` |
| struct | `StagePolicy` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:137` |
| struct | `FallbackPolicy` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:144` |
| struct | `PlungeStressWarning` | `crates/rs_cam_core/src/tool_load/plunge_stress.rs:44` |
| fn | `milling_criteria` | `crates/rs_cam_core/src/tool_load/verdict.rs:345` |
| fn | `trace_polygon_at_z_reported` | `crates/rs_cam_core/src/trace.rs:60` |
| fn | `trace_toolpath_with_cancel` | `crates/rs_cam_core/src/trace.rs:133` |
| fn | `is_index_preserving` | `crates/rs_cam_core/src/transform_provenance.rs:129` |
| struct | `RoutedLink` | `crates/rs_cam_core/src/unified_finish.rs:259` |
| fn | `link_rate` | `crates/rs_cam_core/src/unified_finish.rs:1289` |
| fn | `zigzag_toolpath_reported` | `crates/rs_cam_core/src/zigzag.rs:57` |
| const | `DEFAULT_MAX_SEMANTIC_SUMMARIES` | `crates/rs_cam_mcp/src/response.rs:66` |
| const | `DEFAULT_MAX_DRILL_SAMPLES` | `crates/rs_cam_mcp/src/response.rs:69` |
| fn | `try_charge` | `crates/rs_cam_mcp/src/response.rs:241` |
| fn | `insert_section` | `crates/rs_cam_mcp/src/response.rs:446` |
| struct | `GenerateAllScope` | `crates/rs_cam_viz/src/controller/generate_all.rs:17` |
| fn | `single_pass` | `crates/rs_cam_viz/src/controller/generate_all.rs:224` |
| fn | `load_preset` | `crates/rs_cam_viz/src/io/presets.rs:69` |
| fn | `delete_preset` | `crates/rs_cam_viz/src/io/presets.rs:82` |
| fn | `pump_age` | `crates/rs_cam_viz/src/mcp_bridge.rs:239` |
| fn | `current_frame_age` | `crates/rs_cam_viz/src/mcp_bridge.rs:318` |
| type | `ToolpathSnapshot` | `crates/rs_cam_viz/src/state/history.rs:65` |
| fn | `transform_info` | `crates/rs_cam_viz/src/state/job.rs:76` |
| struct | `OptimizeStageRow` | `crates/rs_cam_viz/src/state/mod.rs:412` |
| fn | `ladder_tier_strategies` | `crates/rs_cam_viz/src/state/multitool_planner.rs:346` |
| fn | `setup_candidates` | `crates/rs_cam_viz/src/state/rest_dependency.rs:44` |
| struct | `SimulationRuntimeHotspot` | `crates/rs_cam_viz/src/state/simulation.rs:103` |
| struct | `ActiveCutSample` | `crates/rs_cam_viz/src/state/simulation.rs:117` |
| fn | `holder_collision_counts_by_tp` | `crates/rs_cam_viz/src/state/simulation.rs:1295` |
| fn | `boundary_for_toolpath_id` | `crates/rs_cam_viz/src/state/simulation.rs:1309` |
| fn | `semantic_runtime_metrics` | `crates/rs_cam_viz/src/state/simulation.rs:1371` |
| fn | `current_cut_sample` | `crates/rs_cam_viz/src/state/simulation.rs:1476` |
| fn | `runtime_hotspots` | `crates/rs_cam_viz/src/state/simulation.rs:1499` |
| fn | `playback_semantic_item` | `crates/rs_cam_viz/src/state/simulation.rs:1565` |
| fn | `semantic_item_by_id` | `crates/rs_cam_viz/src/state/simulation.rs:1584` |
| fn | `trace_target_for_span` | `crates/rs_cam_viz/src/state/simulation.rs:1681` |
| fn | `current_debug_annotation_with_index` | `crates/rs_cam_viz/src/state/simulation.rs:1813` |
| fn | `new_toolpath` | `crates/rs_cam_viz/src/state/toolpath/entry.rs:72` |
| fn | `from_loaded_state` | `crates/rs_cam_viz/src/state/toolpath/entry.rs:88` |
| fn | `duplicate_from` | `crates/rs_cam_viz/src/state/toolpath/entry.rs:98` |
| fn | `duplicate_as` | `crates/rs_cam_viz/src/state/toolpath/entry.rs:264` |
| fn | `clear_runtime_state` | `crates/rs_cam_viz/src/state/toolpath/entry.rs:268` |
| struct | `ToolpathMoveVisibility` | `crates/rs_cam_viz/src/state/viewport.rs:41` |
| fn | `pressed_fill` | `crates/rs_cam_viz/src/ui/components/button.rs:50` |
| struct | `FocusRing` | `crates/rs_cam_viz/src/ui/components/focus_ring.rs:43` |
| fn | `paint_ring` | `crates/rs_cam_viz/src/ui/components/focus_ring.rs:79` |
| const | `DEFAULT_CAP` | `crates/rs_cam_viz/src/ui/components/notice.rs:120` |
| struct | `ApplyReport` | `crates/rs_cam_viz/src/ui/overlays/registry.rs:1324` |
| fn | `apply_workspace_defaults` | `crates/rs_cam_viz/src/ui/overlays/registry.rs:1411` |
| fn | `model_profile` | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:1616` |
| fn | `record_click` | `crates/rs_cam_viz/src/ui/properties/pills.rs:112` |
| fn | `trace_availability_badge` | `crates/rs_cam_viz/src/ui/sim_debug.rs:12` |
| const | `SIZE_HEADING` | `crates/rs_cam_viz/src/ui/tokens.rs:329` |
| const | `SIZE_BUTTON` | `crates/rs_cam_viz/src/ui/tokens.rs:333` |
| const | `SIZE_NUMERIC` | `crates/rs_cam_viz/src/ui/tokens.rs:335` |
| const | `SIZE_DISPLAY` | `crates/rs_cam_viz/src/ui/tokens.rs:344` |
| const | `SIZE_SUBHEAD` | `crates/rs_cam_viz/src/ui/tokens.rs:346` |
| type | `SelectArgs` | `crates/rs_cam_viz/src/ui_command.rs:72` |
| type | `SetViewPresetArgs` | `crates/rs_cam_viz/src/ui_command.rs:75` |
| type | `PreviewOrientationArgs` | `crates/rs_cam_viz/src/ui_command.rs:78` |
| type | `SwitchWorkspaceArgs` | `crates/rs_cam_viz/src/ui_command.rs:81` |
| type | `ToggleToolpathVisibilityArgs` | `crates/rs_cam_viz/src/ui_command.rs:84` |
| type | `InspectToolpathInSimulationArgs` | `crates/rs_cam_viz/src/ui_command.rs:87` |
| type | `SetProjectFeedsOpenArgs` | `crates/rs_cam_viz/src/ui_command.rs:193` |
| type | `SetFeedsProjectSortArgs` | `crates/rs_cam_viz/src/ui_command.rs:196` |
| type | `SetFeedsExploreArgs` | `crates/rs_cam_viz/src/ui_command.rs:199` |
| type | `ToggleFeedsProjectRowArgs` | `crates/rs_cam_viz/src/ui_command.rs:202` |
| type | `SetFeedsProjectScatterArgs` | `crates/rs_cam_viz/src/ui_command.rs:205` |
| type | `SetFeedsProjectSelectAllArgs` | `crates/rs_cam_viz/src/ui_command.rs:208` |
| type | `ToggleOptimizeProjectRowArgs` | `crates/rs_cam_viz/src/ui_command.rs:223` |
| type | `SetStaleExportPolicyArgs` | `crates/rs_cam_viz/src/ui_command.rs:226` |
| type | `CreateToolCatalogArgs` | `crates/rs_cam_viz/src/ui_command.rs:260` |
| type | `DeleteToolCatalogArgs` | `crates/rs_cam_viz/src/ui_command.rs:263` |
| type | `DedupeToolCatalogArgs` | `crates/rs_cam_viz/src/ui_command.rs:275` |
| type | `SaveMachineToLibraryArgs` | `crates/rs_cam_viz/src/ui_command.rs:278` |
| type | `DeleteMachineFromLibraryArgs` | `crates/rs_cam_viz/src/ui_command.rs:281` |
| type | `ListToolCatalogArgs` | `crates/rs_cam_viz/src/ui_command.rs:331` |
