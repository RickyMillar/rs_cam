# Inventory (mechanical)

## Largest production files (top 25)

| lines | file |
|---|---|
| 8011 | `crates/rs_cam_core/src/session/compute.rs` |
| 6660 | `crates/rs_cam_viz/src/controller/tests.rs` |
| 6354 | `crates/rs_cam_core/src/compute/execute.rs` |
| 6053 | `crates/rs_cam_viz/src/app/mcp.rs` |
| 5984 | `crates/rs_cam_viz/src/ui/properties/mod.rs` |
| 5731 | `crates/rs_cam_core/src/feeds/suggest.rs` |
| 4799 | `crates/rs_cam_core/src/unified_finish.rs` |
| 4718 | `crates/rs_cam_core/src/dressup.rs` |
| 4622 | `crates/rs_cam_core/src/feeds/mod.rs` |
| 4402 | `crates/rs_cam_core/src/conformal_spiral.rs` |
| 3645 | `crates/rs_cam_core/src/pencil.rs` |
| 3588 | `crates/rs_cam_core/src/scallop.rs` |
| 3439 | `crates/rs_cam_viz/src/state/simulation.rs` |
| 3359 | `crates/rs_cam_core/src/tool_load/optimize/mod.rs` |
| 3324 | `crates/rs_cam_core/src/compute/catalog.rs` |
| 3314 | `crates/rs_cam_core/src/session/mutation.rs` |
| 3134 | `crates/rs_cam_core/src/simulation_cut.rs` |
| 3026 | `crates/rs_cam_core/src/adaptive3d/mod.rs` |
| 3019 | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` |
| 2999 | `crates/rs_cam_core/src/polygon.rs` |
| 2833 | `crates/rs_cam_core/src/session/mod.rs` |
| 2823 | `crates/rs_cam_core/src/rest_field.rs` |
| 2727 | `crates/rs_cam_core/src/session/command.rs` |
| 2698 | `crates/rs_cam_core/src/compute/operation_configs.rs` |
| 2692 | `crates/rs_cam_viz/src/app/mcp/commands.rs` |

## Functions >= 250 lines (crude: to the next `fn`) (89)

| lines | fn | file:line |
|---|---|---|
| 1302 | `calculate` | `crates/rs_cam_core/src/feeds/mod.rs:1192` |
| 1262 | `upload_gpu_data` | `crates/rs_cam_viz/src/app/gpu_upload.rs:158` |
| 1241 | `draw_toolpath_panel` | `crates/rs_cam_viz/src/ui/properties/mod.rs:4051` |
| 1198 | `unified_finish_toolpath_with_cancel_and_ceiling` | `crates/rs_cam_core/src/unified_finish.rs:1510` |
| 1048 | `fmt` | `crates/rs_cam_core/src/session/command.rs:950` |
| 1038 | `clear_z_level_agent_2d_slice` | `crates/rs_cam_core/src/adaptive3d/clearing.rs:1463` |
| 1015 | `strip_all` | `crates/rs_cam_core/src/compute/catalog.rs:1540` |
| 857 | `in_simulation` | `crates/rs_cam_viz/src/ui/overlays/registry.rs:393` |
| 826 | `describe_core` | `crates/rs_cam_viz/src/app/mcp/commands.rs:1823` |
| 801 | `adaptive_3d_segments` | `crates/rs_cam_core/src/adaptive3d/path.rs:237` |
| 684 | `draw` | `crates/rs_cam_viz/src/ui/properties/mod.rs:549` |
| 660 | `evaluate_inner` | `crates/rs_cam_core/src/tool_load/chipload.rs:441` |
| 657 | `simulation_3d_html` | `crates/rs_cam_core/src/viz.rs:694` |
| 633 | `scallop_toolpath_research_with_stage` | `crates/rs_cam_core/src/scallop.rs:2173` |
| 631 | `drain_compute_results` | `crates/rs_cam_viz/src/controller/events/compute.rs:409` |
| 623 | `run_simulation_memoized` | `crates/rs_cam_core/src/compute/simulate.rs:841` |
| 621 | `target_chipload` | `crates/rs_cam_core/src/feeds/suggest.rs:72` |
| 603 | `adaptive_segments_with_debug` | `crates/rs_cam_core/src/adaptive/path.rs:115` |
| 594 | `ring_spacing_margin_mm` | `crates/rs_cam_core/src/conformal_spiral.rs:575` |
| 574 | `stacked_simulation_3d_html` | `crates/rs_cam_core/src/viz.rs:1351` |
| 546 | `relink_fragments_with_kinds` | `crates/rs_cam_core/src/surface_link.rs:662` |
| 539 | `needs_generation` | `crates/rs_cam_core/src/compute/config.rs:126` |
| 537 | `main` | `crates/rs_cam_cli/src/main.rs:421` |
| 535 | `detect_rest_valleys` | `crates/rs_cam_core/src/rest_field.rs:652` |
| 506 | `default` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:151` |
| 496 | `resolve_generation_inputs` | `crates/rs_cam_core/src/session/compute.rs:2650` |
| 491 | `new` | `crates/rs_cam_viz/src/render/mod.rs:240` |
| 491 | `handle_internal_event` | `crates/rs_cam_viz/src/controller/events/mod.rs:149` |
| 482 | `run_project_command` | `crates/rs_cam_cli/src/project.rs:218` |
| 481 | `core_command_for` | `crates/rs_cam_viz/src/app/mcp/commands.rs:326` |
| 475 | `draw_project_section` | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:435` |
| 459 | `apply_dressups` | `crates/rs_cam_core/src/compute/execute.rs:3980` |
| 437 | `draw_frame` | `crates/rs_cam_viz/src/app.rs:706` |
| 430 | `execute_job` | `crates/rs_cam_core/src/session/compute.rs:563` |
| 429 | `draw_viewport` | `crates/rs_cam_viz/src/app/viewport.rs:224` |
| 426 | `diagnostics_with_evidence` | `crates/rs_cam_core/src/session/compute.rs:5049` |
| 424 | `prepare` | `crates/rs_cam_viz/src/render/mod.rs:876` |
| 422 | `handle_events` | `crates/rs_cam_viz/src/app/input.rs:11` |
| 422 | `draw_height_diagram` | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:1677` |
| 418 | `segments_to_toolpath` | `crates/rs_cam_core/src/adaptive3d/path.rs:1237` |
| 417 | `handle_mcp_request` | `crates/rs_cam_viz/src/app/mcp.rs:222` |
| 407 | `stamp_swept_chunk` | `crates/rs_cam_core/src/dexel_stock/swept.rs:398` |
| 405 | `draw_signal_spine` | `crates/rs_cam_viz/src/ui/sim_timeline.rs:265` |
| 402 | `draw` | `crates/rs_cam_viz/src/ui/sim_op_list.rs:19` |
| 390 | `generate_scallop_rings_with_cancel` | `crates/rs_cam_core/src/scallop.rs:1196` |
| 375 | `draw_chart_c` | `crates/rs_cam_viz/src/ui/feeds/explore.rs:364` |
| 372 | `clear_z_level_contour_parallel` | `crates/rs_cam_core/src/adaptive3d/clearing.rs:646` |
| 371 | `entry_for_warning` | `crates/rs_cam_core/src/feeds/rationale.rs:191` |
| 370 | `update_live_sim` | `crates/rs_cam_viz/src/app/simulation.rs:90` |
| 368 | `draw_signal_track` | `crates/rs_cam_viz/src/ui/sim_timeline.rs:670` |
| 358 | `apply_lead_in_out_with_provenance` | `crates/rs_cam_core/src/dressup.rs:1669` |
| 356 | `simulate_toolpath_with_lut_metrics_rapid_checked` | `crates/rs_cam_core/src/dexel_stock/simulation.rs:403` |
| 353 | `stamp_segment_with_metrics` | `crates/rs_cam_core/src/dexel_stock/stamping.rs:1277` |
| 343 | `draw` | `crates/rs_cam_viz/src/ui/properties/setup.rs:36` |
| 334 | `ramp_finish_toolpath_structured_annotated_with_resolution` | `crates/rs_cam_core/src/ramp_finish.rs:546` |
| 330 | `default` | `crates/rs_cam_core/src/feeds/mod.rs:306` |
| 323 | `apply_adaptive_feed_modulation` | `crates/rs_cam_core/src/session/compute.rs:4284` |
| 322 | `evaluate` | `crates/rs_cam_core/src/tool_load/power.rs:261` |
| 318 | `generate_unified_finish` | `crates/rs_cam_core/src/compute/execute.rs:2575` |
| 316 | `modulate_annotated_against_trace` | `crates/rs_cam_core/src/session/compute.rs:2171` |

## `allow(` in production code (725)

- `crates/rs_cam_core/src/ramp_finish.rs:248` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/ramp_finish.rs:277` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/ramp_finish.rs:310` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/ramp_finish.rs:458` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/ramp_finish.rs:483` #[allow(clippy::indexing_slicing, clippy::expect_used)]
- `crates/rs_cam_core/src/ramp_finish.rs:515` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/ramp_finish.rs:545` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/enriched_mesh.rs:139` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/enriched_mesh.rs:304` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/enriched_mesh.rs:310` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/enriched_mesh.rs:330` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/enriched_mesh.rs:344` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/enriched_mesh.rs:346` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/enriched_mesh.rs:478` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dxf_input.rs:302` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dxf_input.rs:324` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dxf_input.rs:513` #[allow(clippy::indexing_slicing)] // len >= 2 guarded below
- `crates/rs_cam_core/src/dxf_input.rs:535` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dxf_input.rs:600` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dxf_input.rs:657` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_mesh.rs:32` #[allow(clippy::indexing_slicing)] // grid indexing bounded by row/col loops
- `crates/rs_cam_core/src/dexel_mesh.rs:188` #[allow(clippy::indexing_slicing)] // grid indexing bounded by row*cols iteration
- `crates/rs_cam_core/src/direction_field.rs:502` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/direction_field.rs:648` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/direction_field.rs:1087` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/direction_field.rs:1152` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/direction_field.rs:1352` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/direction_field.rs:1531` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/pushcutter.rs:184` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/pushcutter.rs:411` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/pushcutter.rs:422` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_mesh_mc.rs:48` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dexel_mesh_mc.rs:343` #[allow(clippy::too_many_arguments, dead_code)]
- `crates/rs_cam_core/src/dexel_mesh_mc.rs:372` #[allow(clippy::too_many_arguments, dead_code)]
- `crates/rs_cam_core/src/dexel_mesh_mc.rs:403` #[allow(clippy::too_many_arguments, dead_code)]
- `crates/rs_cam_core/src/dexel_mesh_mc.rs:440` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:332` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:545` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:953` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:957` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1016` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1021` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1030` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1032` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1051` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1066` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1068` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1390` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1401` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/polygon.rs:1562` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1604` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1643` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1687` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1699` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1767` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1775` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1797` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1811` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1819` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1914` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1953` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/svg_input.rs:151` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/svg_input.rs:154` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/marching_squares.rs:149` #[allow(clippy::indexing_slicing)] // SAFETY: indices bounded by segment count
- `crates/rs_cam_core/src/radial_finish.rs:54` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/stock_mesh.rs:44` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock_mesh.rs:102` #[allow(clippy::indexing_slicing)] // stride-3 loop bounded by num_verts
- `crates/rs_cam_core/src/stock_mesh.rs:227` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock_mesh.rs:289` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock_mesh.rs:416` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/rest_field.rs:561` #[allow(clippy::too_many_arguments)] // one cohesive measurement off five grids
- `crates/rs_cam_core/src/rest_field.rs:777` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/rest_field.rs:1223` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/rest_field.rs:1232` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/rest_field.rs:1341` #[allow(clippy::nonminimal_bool)]
- `crates/rs_cam_core/src/rest_field.rs:1915` #[allow(clippy::indexing_slicing)] // r,c bounded by nx,ny above
- `crates/rs_cam_core/src/rest_field.rs:1935` #[allow(clippy::indexing_slicing)] // all indices bounded by nx/ny by construction
- `crates/rs_cam_core/src/spiral_finish.rs:105` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/spiral_finish.rs:130` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/spiral_finish.rs:166` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/machine.rs:248` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/unified_finish.rs:1457` #[allow(clippy::too_many_arguments)] // op-generator adapter surface, mirrors the strategy fns it composes
- `crates/rs_cam_core/src/unified_finish.rs:1509` #[allow(clippy::too_many_arguments)] // op-generator adapter surface, mirrors the strategy fns it composes
- `crates/rs_cam_core/src/unified_finish.rs:1841` #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
- `crates/rs_cam_core/src/unified_finish.rs:2707` #[allow(clippy::too_many_arguments)] // lattice dials (step, direction, window) ride beside the op params, mirroring the
- `crates/rs_cam_core/src/unified_finish.rs:2759` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/unified_finish.rs:2975` #[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
- `crates/rs_cam_core/src/unified_finish.rs:3052` #[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
- `crates/rs_cam_core/src/unified_finish.rs:3201` #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
- `crates/rs_cam_core/src/debug_trace.rs:218` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/debug_trace.rs:277` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/debug_trace.rs:318` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/debug_trace.rs:353` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/debug_trace.rs:453` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/debug_trace.rs:462` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/collision.rs:232` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/collision.rs:363` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/collision.rs:463` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive_shared.rs:115` #[allow(clippy::indexing_slicing, clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive_shared.rs:121` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive_shared.rs:124` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive_shared.rs:204` #[allow(clippy::indexing_slicing, clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive_shared.rs:211` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive_shared.rs:214` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop_isofield.rs:136` #[allow(clippy::indexing_slicing)] // SAFETY: i = row*cols + col with both bounded by the loops
- `crates/rs_cam_core/src/scallop_isofield.rs:196` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop_isofield.rs:242` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/scallop_isofield.rs:305` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/scallop_isofield.rs:371` #[allow(clippy::indexing_slicing)] // SAFETY: bounds checked before each read
- `crates/rs_cam_core/src/scallop_isofield.rs:427` #[allow(clippy::indexing_slicing)] // SAFETY: row/col bounded by the loop ranges
- `crates/rs_cam_core/src/dexel.rs:41` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:62` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:85` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:129` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:163` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:473` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:480` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:487` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:497` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:505` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:512` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel.rs:521` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/zigzag.rs:63` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/zigzag.rs:77` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/grid_field.rs:21` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by loop variables and n
- `crates/rs_cam_core/src/grid_field.rs:78` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by rows/cols loop variables
- `crates/rs_cam_core/src/grid_field.rs:127` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by row/col loop ranges
- `crates/rs_cam_core/src/grid_field.rs:180` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by row/col loop ranges
- `crates/rs_cam_core/src/edge_distance.rs:85` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/edge_distance.rs:288` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish_planner.rs:1166` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dressup.rs:153` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup.rs:735` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup.rs:769` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup.rs:861` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup.rs:884` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup.rs:919` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup.rs:957` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup.rs:1011` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dressup.rs:1223` #[allow(clippy::indexing_slicing)] // windows(2) pairs, bounded
- `crates/rs_cam_core/src/dressup.rs:1362` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup.rs:1399` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/dressup.rs:1593` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup.rs:1668` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup.rs:2039` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup.rs:2400` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup.rs:2945` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/project_curve.rs:86` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/project_curve.rs:103` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/project_curve.rs:106` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/project_curve.rs:135` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/project_curve.rs:185` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/project_curve.rs:232` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/step_input.rs:43` #[allow(clippy::indexing_slicing)] // mesh vertex/face indices bounded by tessellation output
- `crates/rs_cam_core/src/step_input.rs:241` #[allow(clippy::indexing_slicing)] // triangle indices [0..3] and vertex lookups bounded by mesh
- `crates/rs_cam_core/src/step_input.rs:348` #[allow(clippy::indexing_slicing)] // vertex indices bounded by mesh topology
- `crates/rs_cam_core/src/step_input.rs:411` #[allow(clippy::indexing_slicing)] // vertex indices bounded by face tessellation
- `crates/rs_cam_core/src/dropcutter.rs:34` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dropcutter.rs:88` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dropcutter.rs:98` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/dropcutter.rs:508` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dropcutter.rs:537` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/crease_paths.rs:97` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/pocket.rs:52` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/pocket.rs:137` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/pocket.rs:241` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/simulation_cut.rs:1481` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/classify_probe.rs:346` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/classify_probe.rs:357` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/classify_probe.rs:399` #[allow(clippy::indexing_slicing)] // SAFETY: every index is derived from the tile bounds below
- `crates/rs_cam_core/src/classify_probe.rs:536` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/classify_probe.rs:556` #[allow(clippy::indexing_slicing)] // SAFETY: tile bounds are derived from spec.rows/cols
- `crates/rs_cam_core/src/semantic_trace.rs:576` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/semantic_trace.rs:604` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/semantic_trace.rs:636` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/semantic_trace.rs:656` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/semantic_trace.rs:772` #[allow(clippy::indexing_slicing)] // bounds checked on the line above each index
- `crates/rs_cam_core/src/semantic_trace.rs:910` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/semantic_trace.rs:988` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/strategy_advisor.rs:171` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/steep_shallow.rs:84` #[allow(clippy::indexing_slicing)] // bounded indexing in grid morphology
- `crates/rs_cam_core/src/steep_shallow.rs:132` #[allow(
- `crates/rs_cam_core/src/steep_shallow.rs:184` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/steep_shallow.rs:366` #[allow(
- `crates/rs_cam_core/src/steep_shallow.rs:414` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/steep_shallow.rs:568` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/steep_shallow.rs:674` #[allow(clippy::too_many_arguments)] // mirrors the sibling above, plus the split
- `crates/rs_cam_core/src/steep_shallow.rs:704` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/fiber.rs:119` #[allow(clippy::indexing_slicing)] // SAFETY: indices bounded by len checks
- `crates/rs_cam_core/src/toolpath.rs:199` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:215` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:241` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:277` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:332` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:496` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:594` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:772` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)] // bounded by grid dimensions
- `crates/rs_cam_core/src/toolpath.rs:866` #[allow(clippy::indexing_slicing)] // first element access guarded by is_empty check
- `crates/rs_cam_core/src/boundary.rs:454` #[allow(clippy::indexing_slicing)] // bounded by grid dimensions computed from mesh bbox
- `crates/rs_cam_core/src/boundary.rs:492` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/boundary.rs:564` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feed_modulation.rs:344` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feed_modulation.rs:730` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine_kinematics.rs:659` #[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
- `crates/rs_cam_core/src/machine_kinematics.rs:781` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine_kinematics.rs:823` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine_kinematics.rs:975` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine_kinematics.rs:1012` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine_kinematics.rs:1161` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/face.rs:98` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/face.rs:101` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/face.rs:108` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/face.rs:130` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/tsp.rs:142` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/tsp.rs:149` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/tsp.rs:208` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/tsp.rs:310` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/tsp.rs:355` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/tsp.rs:465` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/tsp.rs:532` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/tsp.rs:600` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/pencil.rs:345` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/pencil.rs:421` #[allow(clippy::indexing_slicing)] // i bounded to 1..len-1; neighbours i±1 valid
- `crates/rs_cam_core/src/pencil.rs:581` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/pencil.rs:596` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/pencil.rs:787` #[allow(clippy::too_many_arguments)] // cohesive per-chain emit; splitting hurts clarity
- `crates/rs_cam_core/src/pencil.rs:967` #[allow(clippy::indexing_slicing)] // i < n by loop guard
- `crates/rs_cam_core/src/pencil.rs:985` #[allow(clippy::indexing_slicing)] // depths non-empty → index < len
- `crates/rs_cam_core/src/pencil.rs:1459` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/pencil.rs:1905` #[allow(clippy::expect_used, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/pencil.rs:2129` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/pencil.rs:2335` #[allow(clippy::indexing_slicing)] // face_a/face_b are valid mesh face indices
- `crates/rs_cam_core/src/pencil.rs:2407` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/mesh.rs:40` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:137` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:225` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:282` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:384` #[allow(clippy::indexing_slicing)] // bounded by triangle indices from source mesh
- `crates/rs_cam_core/src/mesh.rs:416` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:519` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:633` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:739` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:779` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/trace.rs:122` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/crest_lines.rs:220` #[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
- `crates/rs_cam_core/src/crest_lines.rs:275` #[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
- `crates/rs_cam_core/src/crest_lines.rs:318` #[allow(clippy::indexing_slicing)] // fixed 4×4 literal indices
- `crates/rs_cam_core/src/crest_lines.rs:327` #[allow(clippy::indexing_slicing)] // all indices are mesh vertex/face indices or fixed 0..3
- `crates/rs_cam_core/src/crest_lines.rs:567` #[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
- `crates/rs_cam_core/src/crest_lines.rs:600` #[allow(clippy::indexing_slicing)] // tri/loop indices bounded to mesh data and 0..3
- `crates/rs_cam_core/src/horizontal_finish.rs:69` #[allow(clippy::indexing_slicing, clippy::expect_used)]
- `crates/rs_cam_core/src/horizontal_finish.rs:92` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)] // mesh vertex/face indexing is bounded by mesh structure
- `crates/rs_cam_core/src/horizontal_finish.rs:302` #[allow(clippy::indexing_slicing)] // tri_idx bounded by flat_face_set.len() check
- `crates/rs_cam_core/src/conformal_spiral.rs:1182` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/conformal_spiral.rs:1280` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/conformal_spiral.rs:1525` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/conformal_spiral.rs:2546` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/conformal_spiral.rs:2664` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/conformal_spiral.rs:2776` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/conformal_spiral.rs:3133` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feedopt.rs:277` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/slope.rs:112` #[allow(clippy::too_many_arguments, clippy::expect_used)]
- `crates/rs_cam_core/src/slope.rs:140` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/slope.rs:242` #[allow(clippy::too_many_arguments, clippy::panic)]
- `crates/rs_cam_core/src/slope.rs:271` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/slope.rs:294` #[allow(clippy::indexing_slicing)] // bounds checked on the line above
- `crates/rs_cam_core/src/slope.rs:335` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/slope.rs:425` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/slope.rs:487` #[allow(clippy::indexing_slicing)] // SAFETY: idx bounded by caller loop ranges
- `crates/rs_cam_core/src/slope.rs:488` #[allow(clippy::needless_pass_by_value)] // tuple of mut refs is the natural pattern
- `crates/rs_cam_core/src/slope.rs:504` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/slope.rs:533` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/slope.rs:565` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/slope.rs:576` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/slope.rs:588` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/slope.rs:598` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/slope.rs:651` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/slope.rs:731` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/slope.rs:738` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/slope.rs:745` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/slope.rs:752` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/slope.rs:759` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/flow_accum.rs:24` //! are therefore bounded by construction; the `#[allow(clippy::indexing_slicing)]`
- `crates/rs_cam_core/src/flow_accum.rs:83` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/flow_accum.rs:130` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/flow_accum.rs:195` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/flow_accum.rs:341` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/flow_accum.rs:374` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/rest.rs:145` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/depth.rs:219` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/depth.rs:288` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/waterline.rs:63` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/waterline.rs:133` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/waterline.rs:172` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/contour_extract.rs:65` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/contour_extract.rs:94` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/contour_extract.rs:148` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/contour_extract.rs:170` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/contour_extract.rs:268` #[allow(clippy::indexing_slicing)] // SAFETY: row/col bounded by loop ranges checked above
- `crates/rs_cam_core/src/geo.rs:260` #[allow(clippy::indexing_slicing)] // windows(2) yields len-2 slices
- `crates/rs_cam_core/src/geo.rs:365` #[allow(clippy::unwrap_used, clippy::panic)] // Tests: unwrap is idiomatic for asserting success
- `crates/rs_cam_core/src/grid2.rs:21` //! raw slice indexing anywhere in this module, so no `#[allow(clippy::indexing_slicing)]`
- `crates/rs_cam_core/src/narrate.rs:898` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/narrate.rs:904` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/arc_util.rs:81` #[allow(clippy::unwrap_used, clippy::panic)] // Tests: unwrap is idiomatic for asserting success
- `crates/rs_cam_core/src/inlay.rs:208` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/inlay.rs:221` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/inlay.rs:292` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/radial_profile.rs:116` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/radial_profile.rs:118` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/condition.rs:55` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/vcarve.rs:70` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/vcarve.rs:83` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/vcarve.rs:138` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/viz.rs:52` #[allow(clippy::indexing_slicing)] // i starts at 1, so i-1 is always valid
- `crates/rs_cam_core/src/viz.rs:234` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/viz.rs:285` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/viz.rs:482` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/viz.rs:687` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/viz.rs:1345` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/viz.rs:1930` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/viz.rs:1945` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/viz.rs:1971` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/viz.rs:1987` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/pencil_dihedral.rs:54` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/pencil_dihedral.rs:72` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/pencil_dihedral.rs:139` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/pencil_dihedral.rs:346` #[allow(clippy::indexing_slicing)] // chain entries are valid vertex indices
- `crates/rs_cam_core/src/fingerprint.rs:1108` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/fingerprint.rs:1155` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/fingerprint.rs:1269` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/fingerprint.rs:1341` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/fingerprint.rs:1448` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/fingerprint.rs:1473` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/fingerprint.rs:1521` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/arcfit.rs:74` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/arcfit.rs:315` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/arcfit.rs:484` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop.rs:266` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop.rs:268` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop.rs:318` #[allow(clippy::indexing_slicing)] // SAFETY: caller guarantees non-empty
- `crates/rs_cam_core/src/scallop.rs:947` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop.rs:1029` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/scallop.rs:1120` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop.rs:1127` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop.rs:1135` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop.rs:1156` #[allow(clippy::too_many_arguments, clippy::expect_used)]
- `crates/rs_cam_core/src/scallop.rs:1195` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/scallop.rs:1262` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop.rs:1609` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/scallop.rs:1853` #[allow(clippy::indexing_slicing)] // ring/filtered indexing is guarded by len checks
- `crates/rs_cam_core/src/scallop.rs:1875` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/scallop.rs:1908` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/scallop.rs:1948` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/scallop.rs:1987` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/scallop.rs:2096` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/scallop.rs:2132` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/scallop.rs:2172` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/scallop.rs:2486` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface_link.rs:661` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/diagnostics/tests.rs:5` #![allow(
- `crates/rs_cam_core/src/diagnostics/adapters/from_feeds.rs:411` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/mutation.rs:974` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:1152` #[allow(dead_code)]
- `crates/rs_cam_core/src/session/compute.rs:1208` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_core/src/session/compute.rs:1301` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/session/compute.rs:2170` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/compute.rs:2305` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:2323` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:2329` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:2332` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:2335` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:3308` // rather than `#[allow(clippy::unwrap_used)]`.
- `crates/rs_cam_core/src/session/compute.rs:3387` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:3562` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/compute.rs:3763` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/compute.rs:4155` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/session/compute.rs:4166` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/band.rs:80` #[allow(clippy::indexing_slicing)] // caller derives `local` from clamped bounds
- `crates/rs_cam_core/src/dexel_stock/mod.rs:546` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/mod.rs:561` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/swept.rs:195` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:248` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:249` #[allow(clippy::indexing_slicing)] // bounded by clamped bbox / bin count
- `crates/rs_cam_core/src/dexel_stock/swept.rs:270` #[allow(clippy::needless_range_loop)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:396` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:397` #[allow(clippy::indexing_slicing)] // bounded by clamped bbox / bin count
- `crates/rs_cam_core/src/dexel_stock/swept.rs:631` #[allow(clippy::needless_range_loop)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:666` #[allow(clippy::needless_range_loop)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:715` #[allow(clippy::needless_range_loop)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:932` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/whole_path.rs:424` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:217` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:528` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:539` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:631` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:639` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:796` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:797` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:1230` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:1231` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:1703` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/tile_mip.rs:194` #[allow(clippy::indexing_slicing)] // bounded by rows/cols, checked above
- `crates/rs_cam_core/src/dexel_stock/tile_mip.rs:218` #[allow(clippy::indexing_slicing)] // tile indices derived from clamped cells
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:119` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:120` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:265` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:316` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:354` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:401` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:402` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:758` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:859` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1021` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1135` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1170` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1216` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1361` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1397` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1605` #[allow(clippy::indexing_slicing)] // bounded by the loop range
- `crates/rs_cam_core/src/adaptive3d/mod.rs:409` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive3d/mod.rs:452` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive3d/mod.rs:479` #[allow(clippy::type_complexity)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:76` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:131` #[allow(dead_code)] // Some fields are strategy-specific and only read by some strategies.
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:147` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:263` #[allow(dead_code)] // Some fields are strategy-specific (ContourParallel, Adaptive, AgentSearch-2d).
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:334` #[allow(clippy::indexing_slicing)] // SAFETY: padded grid indices bounded by loop ranges
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:485` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:591` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:645` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:784` #[allow(clippy::indexing_slicing)] // idx < path_3d.len() by construction
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:923` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1017` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1193` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1199` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1369` #[allow(clippy::indexing_slicing)] // anchors indexed by enumerate idx
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1402` #[allow(clippy::indexing_slicing)] // SAFETY: i < n, (i+1) % n < n
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1404` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1462` #[allow(clippy::too_many_arguments, clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1619` #[allow(clippy::indexing_slicing)] // order entries are valid region indices
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1979` #[allow(clippy::indexing_slicing)] // checked non-empty above
- `crates/rs_cam_core/src/adaptive3d/path.rs:191` #[allow(dead_code)]
- `crates/rs_cam_core/src/adaptive3d/path.rs:194` #[allow(dead_code)]
- `crates/rs_cam_core/src/adaptive3d/path.rs:203` #[allow(clippy::type_complexity)] // diagnostic probe; returns raw cell-grid metadata
- `crates/rs_cam_core/src/adaptive3d/path.rs:236` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/path.rs:1080` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/search.rs:41` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/search.rs:99` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/search.rs:132` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/search.rs:262` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/tool_load/mod.rs:416` #[allow(clippy::struct_field_names)]
- `crates/rs_cam_core/src/tool_load/chipload.rs:705` #[allow(clippy::indexing_slicing)] // SAFETY: non-empty checked above
- `crates/rs_cam_core/src/tool_load/chipload.rs:895` #[allow(clippy::indexing_slicing)] // SAFETY: median_idx < len() by construction
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs:570` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs:648` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/tool_load/optimize/patches.rs:11` #[allow(unused_imports)]
- `crates/rs_cam_core/src/tool_load/optimize/patches.rs:83` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/tool_load/optimize/retarget_reconciliation_a8.rs:47` #![allow(
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:155` #[allow(dead_code)]
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:157` #[allow(dead_code)]
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:159` #[allow(dead_code)]
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:168` #[allow(dead_code)]
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:193` #[allow(dead_code)]
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:216` #[allow(dead_code)]
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:218` #[allow(dead_code)]
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:220` #[allow(dead_code)]
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:225` #[allow(dead_code)]
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:579` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/feeds/suggest.rs:895` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest.rs:1026` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest.rs:1054` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest.rs:1082` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest.rs:1198` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest.rs:1242` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:188` #[allow(clippy::indexing_slicing)] // best.i stored from this same iteration
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:199` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:611` #[allow(clippy::indexing_slicing)] // best.i stored from this same iteration
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:623` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feeds/provenance.rs:289` #[allow(dead_code)] // symmetry; scallop is stamped via struct fields, not set()
- `crates/rs_cam_core/src/metrology/ownership.rs:97` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/metrology/monge.rs:147` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:180` #[allow(clippy::needless_range_loop, clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:242` #[allow(clippy::needless_range_loop, clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:327` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:500` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:519` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:539` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive/mod.rs:225` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive/path.rs:113` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:547` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive/path.rs:717` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:805` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:909` #[allow(clippy::indexing_slicing)] // i < filtered.len()
- `crates/rs_cam_core/src/adaptive/path.rs:948` #[allow(clippy::indexing_slicing)] // machinable_vec non-empty checked above
- `crates/rs_cam_core/src/adaptive/path.rs:987` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:1142` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:1208` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive/path.rs:1277` #[allow(clippy::indexing_slicing)] // path.len() >= 2 checked above
- `crates/rs_cam_core/src/adaptive/path.rs:1279` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive/path.rs:1338` #[allow(clippy::indexing_slicing)] // i < contour.len() bounded above
- `crates/rs_cam_core/src/adaptive/path.rs:1360` #[allow(clippy::indexing_slicing)] // len >= 2 checked
- `crates/rs_cam_core/src/adaptive/path.rs:1365` #[allow(clippy::indexing_slicing)] // len >= 2 checked
- `crates/rs_cam_core/src/adaptive/path.rs:1384` #[allow(clippy::indexing_slicing)] // best_idx in 0..verts.len()
- `crates/rs_cam_core/src/adaptive/path.rs:1386` #[allow(clippy::indexing_slicing)] // best_idx in 0..verts.len()
- `crates/rs_cam_core/src/adaptive/path.rs:1404` #[allow(clippy::indexing_slicing)] // contour.len() >= 2 checked above
- `crates/rs_cam_core/src/adaptive/path.rs:1411` #[allow(clippy::indexing_slicing)] // i, (i+1)%n bounded by contour len
- `crates/rs_cam_core/src/adaptive/path.rs:1413` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive/path.rs:1431` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/spiral.rs:53` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive/spiral.rs:54` #[allow(clippy::indexing_slicing)] // bounded indexing over grid/loop buffers
- `crates/rs_cam_core/src/adaptive/spiral.rs:256` #[allow(clippy::indexing_slicing)] // fixed-size direction/loop sampling
- `crates/rs_cam_core/src/adaptive/spiral.rs:307` #[allow(clippy::indexing_slicing)] // padded-grid indices bounded by construction
- `crates/rs_cam_core/src/adaptive/spiral.rs:348` #[allow(clippy::indexing_slicing)] // loop indices bounded by len
- `crates/rs_cam_core/src/adaptive/search.rs:18` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:29` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:68` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:108` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:124` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:161` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:215` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:244` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:306` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:317` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/compute/stats.rs:26` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/semantic_helpers.rs:20` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/semantic_helpers.rs:53` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/semantic_helpers.rs:126` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/simulate.rs:1545` #[allow(clippy::too_many_arguments)] // deviation-pass plumbing, mirrors the call site's request fields
- `crates/rs_cam_core/src/compute/simulate.rs:1639` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/simulate.rs:1692` #[allow(clippy::indexing_slicing)] // triangle indices bounded by mesh
- `crates/rs_cam_core/src/compute/execute.rs:3154` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/execute.rs:3356` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/execute.rs:3405` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/execute.rs:3497` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/execute.rs:3678` #[allow(clippy::too_many_arguments)] // post-generation attach point; every
- `crates/rs_cam_core/src/compute/execute.rs:3979` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/execute.rs:4522` #[allow(
- `crates/rs_cam_core/src/compute/execute.rs:4532` #[allow(
- `crates/rs_cam_core/src/compute/execute.rs:4542` #[allow(
- `crates/rs_cam_core/src/compute/execute.rs:4551` #[allow(
- `crates/rs_cam_core/src/gcode/post.rs:344` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/gcode/post.rs:353` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/gcode/post.rs:362` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/gcode/post.rs:371` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/gcode/emitter.rs:334` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/mcp_server.rs:86` #[allow(dead_code)]
- `crates/rs_cam_viz/src/mcp_server.rs:488` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_viz/src/mcp_server.rs:558` #[allow(clippy::needless_pass_by_value)] Parameters(CutTraceParam {
- `crates/rs_cam_viz/src/mcp_server.rs:604` #[allow(clippy::needless_pass_by_value)] Parameters(GenDebugTraceParam {
- `crates/rs_cam_viz/src/mcp_server.rs:882` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_viz/src/mcp_server.rs:908` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_viz/src/mcp_server.rs:937` #[allow(clippy::needless_pass_by_value)] Parameters(AddToolpathViaGuiParam {
- `crates/rs_cam_viz/src/mcp_server.rs:957` #[allow(clippy::needless_pass_by_value)] Parameters(GetNotificationsParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1101` #[allow(clippy::needless_pass_by_value)] Parameters(spec): Parameters<
- `crates/rs_cam_viz/src/mcp_server.rs:1117` #[allow(clippy::needless_pass_by_value)] Parameters(spec): Parameters<PreviewTierMapParam>,
- `crates/rs_cam_viz/src/mcp_server.rs:1228` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_viz/src/mcp_server.rs:1248` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_viz/src/mcp_server.rs:1272` #[allow(clippy::needless_pass_by_value)] Parameters(GenerateAllParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1480` #[allow(clippy::needless_pass_by_value)] Parameters(ScreenshotSimParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1508` #[allow(clippy::needless_pass_by_value)] Parameters(ScreenshotToolpathParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1564` #[allow(clippy::needless_pass_by_value)] Parameters(ScreenshotGuiParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1588` #[allow(clippy::needless_pass_by_value)] Parameters(SetUiViewParam {
- `crates/rs_cam_viz/src/controller.rs:5` #[allow(
- `crates/rs_cam_viz/src/controller.rs:14` #[allow(
- `crates/rs_cam_viz/src/controller.rs:22` #[allow(
- `crates/rs_cam_viz/src/controller.rs:31` #[allow(
- `crates/rs_cam_viz/src/controller.rs:40` #[allow(
- `crates/rs_cam_viz/src/controller.rs:48` #[allow(
- `crates/rs_cam_viz/src/render/sim_render.rs:81` #[allow(clippy::indexing_slicing)] // stride loop bounded by colors.len()
- `crates/rs_cam_viz/src/render/sim_render.rs:144` #[allow(clippy::indexing_slicing)] // stride-3 loop bounded by num_verts = vertices.len()/3
- `crates/rs_cam_viz/src/render/sim_render.rs:237` #[allow(clippy::indexing_slicing)] // triangle stride-3 access bounded by loop
- `crates/rs_cam_viz/src/render/sim_render.rs:326` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/render/sim_render.rs:371` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/render/sim_render.rs:389` #[allow(clippy::indexing_slicing)] // stride-3 loops bounded by vertex/index counts
- `crates/rs_cam_viz/src/render/mod.rs:885` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/render/mod.rs:980` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/render/mod.rs:983` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/render/mod.rs:1308` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/render/colors.rs:21` #[allow(clippy::indexing_slicing)] // modulo indexing into constant-length palette
- `crates/rs_cam_viz/src/render/mesh_render.rs:49` /// zero callers and an `#[allow(dead_code)]`, and P6 deleted it
- `crates/rs_cam_viz/src/render/mesh_render.rs:66` #[allow(clippy::indexing_slicing)] // vertex/triangle indices bounded by mesh invariants
- `crates/rs_cam_viz/src/render/mesh_render.rs:151` #[allow(clippy::indexing_slicing)] // vertex/triangle indices bounded by mesh invariants
- `crates/rs_cam_viz/src/render/mesh_render.rs:222` #[allow(clippy::indexing_slicing)] // modulo indexing into constant-length palette
- `crates/rs_cam_viz/src/render/mesh_render.rs:259` #[allow(clippy::indexing_slicing)] // vertex/triangle indices bounded by mesh invariants
- `crates/rs_cam_viz/src/render/toolpath_render.rs:89` #[allow(clippy::indexing_slicing)] // n - 1 is safe: n > 0 and n <= len
- `crates/rs_cam_viz/src/render/toolpath_render.rs:116` #[allow(clippy::indexing_slicing)] // loop index i bounded by tp.moves.len()
- `crates/rs_cam_viz/src/render/toolpath_render.rs:413` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/render/toolpath_render.rs:567` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/render/toolpath_render.rs:812` #[allow(clippy::indexing_slicing)] // first_cut_idx validated by position() and bounds > 0
- `crates/rs_cam_viz/src/render/toolpath_render.rs:969` #[allow(clippy::indexing_slicing)] // first_cut_idx validated by position() and bounds > 0
- `crates/rs_cam_viz/src/render/camera.rs:87` #[allow(clippy::indexing_slicing)] // nalgebra 4x4 matrix slice is always 16 elements
- `crates/rs_cam_viz/src/render/camera.rs:137` #[allow(clippy::indexing_slicing)] // fixed-size [f32; 3] arrays
- `crates/rs_cam_viz/src/state/history.rs:18` #[allow(clippy::large_enum_variant)]
- `crates/rs_cam_viz/src/state/simulation.rs:1218` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_viz/src/state/simulation.rs:1265` #[allow(clippy::indexing_slicing)] // boundary_index from position() is always in bounds
- `crates/rs_cam_viz/src/state/simulation.rs:1498` #[allow(clippy::indexing_slicing)] // child_index from parent's child list, bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:1564` #[allow(clippy::indexing_slicing)] // active_index from active_item_index() bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:2300` #[allow(clippy::indexing_slicing)] // item_index from enumerate(), bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:2479` #[allow(clippy::indexing_slicing)] // item_index from enumerate(), bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:2519` #[allow(clippy::indexing_slicing)] // item indices from move_item_indices, bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:2543` #[allow(clippy::indexing_slicing)] // index from item_index_by_id, bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:2597` #[allow(clippy::indexing_slicing)] // bounds checked: move_end_exclusive <= cumulative.len()-1
- `crates/rs_cam_viz/src/state/simulation.rs:2620` #[allow(clippy::indexing_slicing)] // move_index bounded by caller's loop over toolpath.moves
- `crates/rs_cam_viz/src/state/job.rs:317` #[allow(clippy::indexing_slicing)] // stride-3 loop bounded by mesh.vertices.len()
- `crates/rs_cam_viz/src/ui/sim_debug.rs:90` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:607` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:741` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:810` #[allow(clippy::too_many_arguments, clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs:135` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs:256` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/preflight.rs:317` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:434` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1414` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1492` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:448` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:564` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:669` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:841` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:848` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:861` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1247` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1536` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1648` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1650` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1652` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1654` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1754` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:2142` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:2143` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:2232` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/export_wizard.rs:620` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/feeds/compare.rs:148` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/feeds/compare.rs:258` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/feeds/shared.rs:45` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:79` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:1732` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:1996` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:2209` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:2348` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:3130` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:4050` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/setup.rs:35` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/stock.rs:282` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/stock.rs:352` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:109` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:1388` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:2072` #[allow(dead_code)] // surfaced via `tool_configs` lookup post-PR-3 cutover
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:2074` #[allow(dead_code)] // surfaced via `tool_configs` lookup post-PR-3 cutover
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:2076` #[allow(dead_code)] // surfaced via `tool_configs` lookup post-PR-3 cutover
- `crates/rs_cam_viz/src/controller/io.rs:529` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/toolpath.rs:287` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:408` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:2070` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:2160` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:2182` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/interaction/picking.rs:301` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:52` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:115` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:165` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:292` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:315` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:405` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:519` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:603` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:864` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:959` #[allow(clippy::type_complexity)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:1371` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/viewport.rs:731` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/simulation.rs:34` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:45` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:89` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/simulation.rs:415` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:426` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:488` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/simulation.rs:607` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/mcp.rs:1337` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/app/mcp.rs:4766` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/app/mcp.rs:4973` #[allow(clippy::indexing_slicing)] // pos came from pass_index_of
- `crates/rs_cam_viz/src/app/mcp.rs:5025` #[allow(clippy::indexing_slicing)] // SAFETY: `kind.index()` is bounded by COUNT.
- `crates/rs_cam_viz/src/app/mcp.rs:5177` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/app/mcp/commands.rs:1916` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/compute/worker.rs:3` #[allow(
- `crates/rs_cam_viz/src/compute/worker.rs:12` #[allow(
- `crates/rs_cam_viz/src/compute/worker.rs:20` #[allow(
- `crates/rs_cam_viz/src/compute/worker.rs:120` #[allow(dead_code)] // populated by caller; consumed by execute pipeline
- `crates/rs_cam_viz/src/compute/worker.rs:345` #[allow(clippy::large_enum_variant)]
- `crates/rs_cam_viz/src/compute/worker.rs:952` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker.rs:1076` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker.rs:1202` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker.rs:1296` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker.rs:1427` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker/gen_parity_p0_tests.rs:272` #[allow(dead_code)]
- `crates/rs_cam_viz/src/compute/worker/helpers.rs:45` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker/test_fixture.rs:233` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_cli/src/nc_replay.rs:1` #![allow(clippy::print_stdout)] // CLI surface
- `crates/rs_cam_cli/src/nc_replay.rs:204` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_cli/src/run.rs:31` #[allow(clippy::struct_excessive_bools)]
- `crates/rs_cam_cli/src/smoke.rs:14` #![allow(clippy::print_stdout)] // CLI surface
- `crates/rs_cam_cli/src/smoke.rs:44` #[allow(dead_code)]
- `crates/rs_cam_cli/src/smoke.rs:479` #[allow(clippy::result_large_err)] // tp_idx tuple is small; failure path is the rare branch
- `crates/rs_cam_cli/src/main.rs:2` #![allow(clippy::print_stderr)] // CLI uses eprintln! for user-facing diagnostic output
- `crates/rs_cam_cli/src/main.rs:3` #![allow(clippy::print_stdout)] // CLI `version` prints build info to stdout
- `crates/rs_cam_cli/src/main.rs:308` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_cli/src/sweep.rs:6` #![allow(clippy::print_stdout, clippy::indexing_slicing)]
- `crates/rs_cam_cli/src/sweep.rs:300` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_cli/src/project.rs:217` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_cli/src/job.rs:185` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:21` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:28` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:37` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:50` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:59` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:505` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:509` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:513` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:519` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:523` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:1008` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:1020` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:1032` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:1040` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:1048` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:1058` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:1375` #[allow(clippy::needless_pass_by_value)]

## TODO / FIXME / XXX / HACK in production code (12)

- `crates/rs_cam_core/src/material.rs:541` /// TODO Phase 3+: replace with a per-species derivation backbone
- `crates/rs_cam_core/src/material.rs:543` /// the `kc.md` TODO trail).
- `crates/rs_cam_core/src/material.rs:844` // TODO deferred), paralleling Phase 2B for sheet goods, which
- `crates/rs_cam_core/src/material.rs:856` // TODO: source from CSIRO or FRI publications.
- `crates/rs_cam_core/src/material.rs:874` // TODO: source from CSIRO publications.
- `crates/rs_cam_core/src/material.rs:877` // retained. TODO: source from EMBRAPA / IPT.
- `crates/rs_cam_core/src/material.rs:888` // TODO Phase 3 — per-grade plywood Kc has no fetched
- `crates/rs_cam_core/src/material.rs:913` // TODO Phase 3: replace with fetched HDF cutting-force
- `crates/rs_cam_core/src/feeds/vendor_normalize.rs:269` // minimal practical impact today. TODO Phase E+: extend
- `crates/rs_cam_core/src/feeds/vendor_normalize.rs:305` // TODO Phase 3+: plumb Rockwell HardnessKind through
- `crates/rs_cam_viz/src/controller/events/toolpath.rs:107` // TODO(v1.2): populate model_bbox + upstream leftover so
- `crates/rs_cam_viz/src/app/mcp/commands.rs:858` // TODO(v1.2): wire model_bbox once add_toolpath via MCP
