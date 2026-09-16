# Inventory (mechanical)

## Largest production files (top 25)

| lines | file |
|---|---|
| 8015 | `crates/rs_cam_core/src/session/compute.rs` |
| 6721 | `crates/rs_cam_viz/src/controller/tests.rs` |
| 6363 | `crates/rs_cam_core/src/compute/execute.rs` |
| 6068 | `crates/rs_cam_viz/src/app/mcp.rs` |
| 5966 | `crates/rs_cam_viz/src/ui/properties/mod.rs` |
| 5770 | `crates/rs_cam_core/src/feeds/suggest.rs` |
| 4807 | `crates/rs_cam_core/src/finish/unified_finish.rs` |
| 4727 | `crates/rs_cam_core/src/dressup/mod.rs` |
| 4622 | `crates/rs_cam_core/src/feeds/mod.rs` |
| 4402 | `crates/rs_cam_core/src/finish/conformal_spiral.rs` |
| 3635 | `crates/rs_cam_core/src/finish/pencil.rs` |
| 3581 | `crates/rs_cam_core/src/finish/scallop.rs` |
| 3362 | `crates/rs_cam_core/src/tool_load/optimize/mod.rs` |
| 3334 | `crates/rs_cam_core/src/compute/catalog.rs` |
| 3303 | `crates/rs_cam_viz/src/state/simulation.rs` |
| 3273 | `crates/rs_cam_core/src/session/mutation.rs` |
| 3142 | `crates/rs_cam_core/src/stock/simulation_cut.rs` |
| 3018 | `crates/rs_cam_core/src/adaptive3d/mod.rs` |
| 3014 | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` |
| 3005 | `crates/rs_cam_core/src/polygon.rs` |
| 2853 | `crates/rs_cam_core/src/session/mod.rs` |
| 2829 | `crates/rs_cam_core/src/surface/rest_field.rs` |
| 2718 | `crates/rs_cam_core/src/compute/operation_configs.rs` |
| 2688 | `crates/rs_cam_viz/src/app/mcp/commands.rs` |
| 2650 | `crates/rs_cam_core/src/adaptive3d/clearing.rs` |

## Functions >= 250 lines (crude: to the next `fn`) (89)

| lines | fn | file:line |
|---|---|---|
| 1302 | `calculate` | `crates/rs_cam_core/src/feeds/mod.rs:1192` |
| 1271 | `upload_gpu_data` | `crates/rs_cam_viz/src/app/gpu_upload.rs:158` |
| 1242 | `draw_toolpath_panel` | `crates/rs_cam_viz/src/ui/properties/mod.rs:4032` |
| 1199 | `unified_finish_toolpath_with_cancel_and_ceiling` | `crates/rs_cam_core/src/finish/unified_finish.rs:1516` |
| 1040 | `clear_z_level_agent_2d_slice` | `crates/rs_cam_core/src/adaptive3d/clearing.rs:1465` |
| 1014 | `strip_all` | `crates/rs_cam_core/src/compute/catalog.rs:1562` |
| 1005 | `fmt` | `crates/rs_cam_core/src/session/command.rs:913` |
| 857 | `in_simulation` | `crates/rs_cam_viz/src/ui/overlays/registry.rs:393` |
| 823 | `describe_core` | `crates/rs_cam_viz/src/app/mcp/commands.rs:1822` |
| 772 | `adaptive_3d_segments` | `crates/rs_cam_core/src/adaptive3d/path.rs:303` |
| 684 | `draw` | `crates/rs_cam_viz/src/ui/properties/mod.rs:532` |
| 660 | `evaluate_inner` | `crates/rs_cam_core/src/tool_load/chipload.rs:441` |
| 638 | `scallop_toolpath_research_with_stage` | `crates/rs_cam_core/src/finish/scallop.rs:2172` |
| 636 | `drain_compute_results` | `crates/rs_cam_viz/src/controller/events/compute.rs:409` |
| 625 | `run_simulation_memoized` | `crates/rs_cam_core/src/compute/simulate.rs:843` |
| 621 | `target_chipload` | `crates/rs_cam_core/src/feeds/suggest.rs:72` |
| 603 | `adaptive_segments_with_debug` | `crates/rs_cam_core/src/adaptive/path.rs:115` |
| 594 | `ring_spacing_margin_mm` | `crates/rs_cam_core/src/finish/conformal_spiral.rs:575` |
| 574 | `stacked_simulation_3d_html` | `crates/rs_cam_core/src/export/viz.rs:580` |
| 548 | `relink_fragments_with_kinds` | `crates/rs_cam_core/src/finish/surface_link.rs:662` |
| 537 | `needs_generation` | `crates/rs_cam_core/src/compute/config.rs:126` |
| 537 | `main` | `crates/rs_cam_cli/src/main.rs:421` |
| 535 | `detect_rest_valleys` | `crates/rs_cam_core/src/surface/rest_field.rs:652` |
| 506 | `default` | `crates/rs_cam_core/src/tool_load/optimize/policy.rs:151` |
| 497 | `resolve_generation_inputs` | `crates/rs_cam_core/src/session/compute.rs:2665` |
| 491 | `new` | `crates/rs_cam_viz/src/render/mod.rs:240` |
| 491 | `handle_internal_event` | `crates/rs_cam_viz/src/controller/events/mod.rs:149` |
| 481 | `core_command_for` | `crates/rs_cam_viz/src/app/mcp/commands.rs:326` |
| 478 | `run_project_command` | `crates/rs_cam_cli/src/project.rs:207` |
| 475 | `draw_project_section` | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:435` |
| 463 | `apply_dressups` | `crates/rs_cam_core/src/compute/execute.rs:3992` |
| 437 | `draw_frame` | `crates/rs_cam_viz/src/app.rs:706` |
| 434 | `execute_job` | `crates/rs_cam_core/src/session/compute.rs:569` |
| 429 | `draw_viewport` | `crates/rs_cam_viz/src/app/viewport.rs:224` |
| 424 | `prepare` | `crates/rs_cam_viz/src/render/mod.rs:876` |
| 422 | `handle_events` | `crates/rs_cam_viz/src/app/input.rs:11` |
| 418 | `segments_to_toolpath` | `crates/rs_cam_core/src/adaptive3d/path.rs:1274` |
| 418 | `draw_height_diagram` | `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:1677` |
| 417 | `handle_mcp_request` | `crates/rs_cam_viz/src/app/mcp.rs:222` |
| 416 | `diagnostics_with_evidence` | `crates/rs_cam_core/src/session/compute.rs:5073` |
| 410 | `draw_signal_spine` | `crates/rs_cam_viz/src/ui/sim_timeline.rs:265` |
| 407 | `stamp_swept_chunk` | `crates/rs_cam_core/src/dexel_stock/swept.rs:398` |
| 402 | `draw` | `crates/rs_cam_viz/src/ui/sim_op_list.rs:19` |
| 390 | `generate_scallop_rings_with_cancel` | `crates/rs_cam_core/src/finish/scallop.rs:1200` |
| 375 | `draw_chart_c` | `crates/rs_cam_viz/src/ui/feeds/explore.rs:364` |
| 372 | `update_live_sim` | `crates/rs_cam_viz/src/app/simulation.rs:90` |
| 372 | `clear_z_level_contour_parallel` | `crates/rs_cam_core/src/adaptive3d/clearing.rs:646` |
| 368 | `draw_signal_track` | `crates/rs_cam_viz/src/ui/sim_timeline.rs:675` |
| 365 | `entry_for_warning` | `crates/rs_cam_core/src/feeds/rationale.rs:193` |
| 358 | `apply_lead_in_out_with_provenance` | `crates/rs_cam_core/src/dressup/mod.rs:1673` |
| 356 | `simulate_toolpath_with_lut_metrics_rapid_checked` | `crates/rs_cam_core/src/dexel_stock/simulation.rs:407` |
| 353 | `stamp_segment_with_metrics` | `crates/rs_cam_core/src/dexel_stock/stamping.rs:1277` |
| 343 | `draw` | `crates/rs_cam_viz/src/ui/properties/setup.rs:36` |
| 337 | `ramp_finish_toolpath_structured_annotated_with_resolution` | `crates/rs_cam_core/src/finish/ramp_finish.rs:532` |
| 330 | `default` | `crates/rs_cam_core/src/feeds/mod.rs:306` |
| 325 | `apply_adaptive_feed_modulation` | `crates/rs_cam_core/src/session/compute.rs:4306` |
| 322 | `evaluate` | `crates/rs_cam_core/src/tool_load/power.rs:261` |
| 317 | `generate_unified_finish` | `crates/rs_cam_core/src/compute/execute.rs:2583` |
| 316 | `modulate_annotated_against_trace` | `crates/rs_cam_core/src/session/compute.rs:2186` |
| 311 | `draw_alignment_pins` | `crates/rs_cam_viz/src/ui/properties/stock.rs:179` |

## `allow(` in production code (665)

- `crates/rs_cam_core/src/polygon.rs:335` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:548` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:956` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:960` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1019` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1024` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1033` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1035` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1054` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1069` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1071` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1393` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1404` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/polygon.rs:1568` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1610` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1649` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1693` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1705` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1773` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1781` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1803` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1817` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1825` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1920` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1959` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/interrupt.rs:34` /// `#[allow(clippy::expect_used)]` over the `Result` it made unreachable.
- `crates/rs_cam_core/src/interrupt.rs:53` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/toolpath.rs:199` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:215` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:241` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:277` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:332` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:496` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:594` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:772` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)] // bounded by grid dimensions
- `crates/rs_cam_core/src/toolpath.rs:866` #[allow(clippy::indexing_slicing)] // first element access guarded by is_empty check
- `crates/rs_cam_core/src/mesh.rs:43` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:140` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:197` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:299` #[allow(clippy::indexing_slicing)] // bounded by triangle indices from source mesh
- `crates/rs_cam_core/src/mesh.rs:331` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:434` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:548` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:654` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/mesh.rs:694` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/geo.rs:256` #[allow(clippy::indexing_slicing)] // SAFETY: windows(2) yields len-2 slices
- `crates/rs_cam_core/src/geo.rs:275` #[allow(clippy::indexing_slicing)] // SAFETY: windows(2) yields len-2 slices
- `crates/rs_cam_core/src/geo.rs:292` #[allow(clippy::indexing_slicing)] // SAFETY: windows(2) yields len-2 slices
- `crates/rs_cam_core/src/geo.rs:397` #[allow(clippy::unwrap_used, clippy::panic)] // Tests: unwrap is idiomatic for asserting success
- `crates/rs_cam_core/src/diagnostics/tests.rs:5` #![allow(
- `crates/rs_cam_core/src/diagnostics/adapters/from_feeds.rs:411` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/mutation.rs:974` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:1162` #[allow(dead_code)]
- `crates/rs_cam_core/src/session/compute.rs:1218` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_core/src/session/compute.rs:1311` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/session/compute.rs:2185` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/compute.rs:2320` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:2338` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:2344` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:2347` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:2350` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:3324` // rather than `#[allow(clippy::unwrap_used)]`.
- `crates/rs_cam_core/src/session/compute.rs:3403` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:3580` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/compute.rs:3783` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/compute.rs:4177` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/session/compute.rs:4188` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/io/dxf_input.rs:302` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/io/dxf_input.rs:324` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/io/dxf_input.rs:513` #[allow(clippy::indexing_slicing)] // len >= 2 guarded below
- `crates/rs_cam_core/src/io/dxf_input.rs:535` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/io/dxf_input.rs:600` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/io/dxf_input.rs:657` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/io/svg_input.rs:114` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/io/svg_input.rs:117` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/io/step_input.rs:43` #[allow(clippy::indexing_slicing)] // mesh vertex/face indices bounded by tessellation output
- `crates/rs_cam_core/src/io/step_input.rs:241` #[allow(clippy::indexing_slicing)] // triangle indices [0..3] and vertex lookups bounded by mesh
- `crates/rs_cam_core/src/io/step_input.rs:348` #[allow(clippy::indexing_slicing)] // vertex indices bounded by mesh topology
- `crates/rs_cam_core/src/io/step_input.rs:411` #[allow(clippy::indexing_slicing)] // vertex indices bounded by face tessellation
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:139` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:278` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:284` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:304` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:318` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:320` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:452` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/marching_squares.rs:149` #[allow(clippy::indexing_slicing)] // SAFETY: indices bounded by segment count
- `crates/rs_cam_core/src/geometry/grid_field.rs:21` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by loop variables and n
- `crates/rs_cam_core/src/geometry/grid_field.rs:78` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by rows/cols loop variables
- `crates/rs_cam_core/src/geometry/grid_field.rs:127` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by row/col loop ranges
- `crates/rs_cam_core/src/geometry/grid_field.rs:180` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by row/col loop ranges
- `crates/rs_cam_core/src/geometry/edge_distance.rs:85` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/edge_distance.rs:288` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/geometry/fiber.rs:119` #[allow(clippy::indexing_slicing)] // SAFETY: indices bounded by len checks
- `crates/rs_cam_core/src/geometry/boundary.rs:455` #[allow(clippy::indexing_slicing)] // bounded by grid dimensions computed from mesh bbox
- `crates/rs_cam_core/src/geometry/boundary.rs:493` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/boundary.rs:565` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/contour_extract.rs:65` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/geometry/contour_extract.rs:94` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/geometry/contour_extract.rs:148` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/geometry/contour_extract.rs:170` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/geometry/contour_extract.rs:268` #[allow(clippy::indexing_slicing)] // SAFETY: row/col bounded by loop ranges checked above
- `crates/rs_cam_core/src/geometry/grid2.rs:21` //! raw slice indexing anywhere in this module, so no `#[allow(clippy::indexing_slicing)]`
- `crates/rs_cam_core/src/geometry/arc_util.rs:81` #[allow(clippy::unwrap_used, clippy::panic)] // Tests: unwrap is idiomatic for asserting success
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
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:123` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:124` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:269` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:320` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:358` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:405` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:406` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:762` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:862` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1024` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1138` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1173` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1219` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1364` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1400` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1608` #[allow(clippy::indexing_slicing)] // bounded by the loop range
- `crates/rs_cam_core/src/ops/adaptive_shared.rs:115` #[allow(clippy::indexing_slicing, clippy::expect_used)]
- `crates/rs_cam_core/src/ops/adaptive_shared.rs:121` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/adaptive_shared.rs:124` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/adaptive_shared.rs:204` #[allow(clippy::indexing_slicing, clippy::expect_used)]
- `crates/rs_cam_core/src/ops/adaptive_shared.rs:211` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/adaptive_shared.rs:214` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/zigzag.rs:66` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/ops/zigzag.rs:80` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/ops/project_curve.rs:86` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/project_curve.rs:103` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/project_curve.rs:106` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/ops/project_curve.rs:183` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/ops/project_curve.rs:230` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/ops/pocket.rs:237` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/ops/face.rs:98` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/face.rs:101` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/face.rs:108` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/rest.rs:149` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/waterline.rs:160` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/ops/inlay.rs:208` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/inlay.rs:221` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/vcarve.rs:70` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/vcarve.rs:83` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/mod.rs:454` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive3d/mod.rs:481` #[allow(clippy::type_complexity)]
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
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1404` #[allow(clippy::indexing_slicing)] // SAFETY: i < n, (i+1) % n < n
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1406` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1464` #[allow(clippy::too_many_arguments, clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1621` #[allow(clippy::indexing_slicing)] // order entries are valid region indices
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1983` #[allow(clippy::indexing_slicing)] // checked non-empty above
- `crates/rs_cam_core/src/adaptive3d/path.rs:34` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/path.rs:257` #[allow(dead_code)]
- `crates/rs_cam_core/src/adaptive3d/path.rs:260` #[allow(dead_code)]
- `crates/rs_cam_core/src/adaptive3d/path.rs:269` #[allow(clippy::type_complexity)] // diagnostic probe; returns raw cell-grid metadata
- `crates/rs_cam_core/src/adaptive3d/path.rs:302` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/path.rs:1117` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/search.rs:41` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/search.rs:99` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/search.rs:132` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/search.rs:262` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/export/viz.rs:50` #[allow(clippy::indexing_slicing)] // i starts at 1, so i-1 is always valid
- `crates/rs_cam_core/src/export/viz.rs:230` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/viz.rs:281` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/export/viz.rs:365` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/export/viz.rs:574` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/export/viz.rs:1159` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/viz.rs:1174` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/viz.rs:1200` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/viz.rs:1216` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/export/fingerprint.rs:1093` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1140` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1254` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1326` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1433` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1458` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1506` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/machine/mod.rs:255` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine/strategy_advisor.rs:171` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/machine/kinematics.rs:665` #[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
- `crates/rs_cam_core/src/machine/kinematics.rs:787` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine/kinematics.rs:829` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine/kinematics.rs:981` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine/kinematics.rs:1018` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine/kinematics.rs:1167` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/tool_load/chipload.rs:705` #[allow(clippy::indexing_slicing)] // SAFETY: non-empty checked above
- `crates/rs_cam_core/src/tool_load/chipload.rs:895` #[allow(clippy::indexing_slicing)] // SAFETY: median_idx < len() by construction
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs:571` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs:652` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/tool_load/optimize/patches.rs:82` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/tool_load/optimize/retarget_reconciliation_a8.rs:47` #![allow(
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:545` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/feeds/suggest.rs:896` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest.rs:1048` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest.rs:1079` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest.rs:1110` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest.rs:1231` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest.rs:1281` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:190` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:201` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:593` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:605` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/ramp_finish.rs:248` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/ramp_finish.rs:277` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/ramp_finish.rs:310` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/ramp_finish.rs:469` #[allow(clippy::indexing_slicing, clippy::expect_used)]
- `crates/rs_cam_core/src/finish/ramp_finish.rs:501` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/ramp_finish.rs:531` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/direction_field.rs:502` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/direction_field.rs:648` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/direction_field.rs:1087` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/direction_field.rs:1152` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/direction_field.rs:1352` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/direction_field.rs:1531` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/spiral_finish.rs:149` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/unified_finish.rs:1463` #[allow(clippy::too_many_arguments)] // op-generator adapter surface, mirrors the strategy fns it composes
- `crates/rs_cam_core/src/finish/unified_finish.rs:1515` #[allow(clippy::too_many_arguments)] // op-generator adapter surface, mirrors the strategy fns it composes
- `crates/rs_cam_core/src/finish/unified_finish.rs:1847` #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
- `crates/rs_cam_core/src/finish/unified_finish.rs:2714` #[allow(clippy::too_many_arguments)] // lattice dials (step, direction, window) ride beside the op params, mirroring the
- `crates/rs_cam_core/src/finish/unified_finish.rs:2766` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/unified_finish.rs:2982` #[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
- `crates/rs_cam_core/src/finish/unified_finish.rs:3059` #[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
- `crates/rs_cam_core/src/finish/unified_finish.rs:3208` #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:136` #[allow(clippy::indexing_slicing)] // SAFETY: i = row*cols + col with both bounded by the loops
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:196` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:242` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:305` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:371` #[allow(clippy::indexing_slicing)] // SAFETY: bounds checked before each read
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:427` #[allow(clippy::indexing_slicing)] // SAFETY: row/col bounded by the loop ranges
- `crates/rs_cam_core/src/finish/finish_planner.rs:1169` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/crease_paths.rs:97` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/classify_probe.rs:349` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/classify_probe.rs:360` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/classify_probe.rs:402` #[allow(clippy::indexing_slicing)] // SAFETY: every index is derived from the tile bounds below
- `crates/rs_cam_core/src/finish/classify_probe.rs:539` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/classify_probe.rs:559` #[allow(clippy::indexing_slicing)] // SAFETY: tile bounds are derived from spec.rows/cols
- `crates/rs_cam_core/src/finish/steep_shallow.rs:84` #[allow(clippy::indexing_slicing)] // bounded indexing in grid morphology
- `crates/rs_cam_core/src/finish/steep_shallow.rs:131` #[allow(
- `crates/rs_cam_core/src/finish/steep_shallow.rs:183` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/steep_shallow.rs:364` #[allow(
- `crates/rs_cam_core/src/finish/steep_shallow.rs:412` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/steep_shallow.rs:670` #[allow(clippy::too_many_arguments)] // mirrors the sibling above, plus the split
- `crates/rs_cam_core/src/finish/steep_shallow.rs:700` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/pencil.rs:345` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/pencil.rs:421` #[allow(clippy::indexing_slicing)] // i bounded to 1..len-1; neighbours i±1 valid
- `crates/rs_cam_core/src/finish/pencil.rs:581` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/pencil.rs:596` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/pencil.rs:789` #[allow(clippy::too_many_arguments)] // cohesive per-chain emit; splitting hurts clarity
- `crates/rs_cam_core/src/finish/pencil.rs:969` #[allow(clippy::indexing_slicing)] // i < n by loop guard
- `crates/rs_cam_core/src/finish/pencil.rs:987` #[allow(clippy::indexing_slicing)] // depths non-empty → index < len
- `crates/rs_cam_core/src/finish/pencil.rs:1461` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/pencil.rs:1906` #[allow(clippy::expect_used, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/pencil.rs:2130` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/pencil.rs:2341` #[allow(clippy::indexing_slicing)] // face_a/face_b are valid mesh face indices
- `crates/rs_cam_core/src/finish/pencil.rs:2413` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/crest_lines.rs:220` #[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
- `crates/rs_cam_core/src/finish/crest_lines.rs:275` #[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
- `crates/rs_cam_core/src/finish/crest_lines.rs:318` #[allow(clippy::indexing_slicing)] // fixed 4×4 literal indices
- `crates/rs_cam_core/src/finish/crest_lines.rs:327` #[allow(clippy::indexing_slicing)] // all indices are mesh vertex/face indices or fixed 0..3
- `crates/rs_cam_core/src/finish/crest_lines.rs:567` #[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
- `crates/rs_cam_core/src/finish/crest_lines.rs:600` #[allow(clippy::indexing_slicing)] // tri/loop indices bounded to mesh data and 0..3
- `crates/rs_cam_core/src/finish/horizontal_finish.rs:68` #[allow(clippy::indexing_slicing, clippy::expect_used)]
- `crates/rs_cam_core/src/finish/horizontal_finish.rs:91` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)] // mesh vertex/face indexing is bounded by mesh structure
- `crates/rs_cam_core/src/finish/horizontal_finish.rs:301` #[allow(clippy::indexing_slicing)] // tri_idx bounded by flat_face_set.len() check
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:1182` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:1280` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:1525` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:2546` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:2664` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:2776` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:3133` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/pencil_dihedral.rs:54` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/pencil_dihedral.rs:72` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/pencil_dihedral.rs:139` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/pencil_dihedral.rs:346` #[allow(clippy::indexing_slicing)] // chain entries are valid vertex indices
- `crates/rs_cam_core/src/finish/scallop.rs:266` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop.rs:268` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop.rs:318` #[allow(clippy::indexing_slicing)] // SAFETY: caller guarantees non-empty
- `crates/rs_cam_core/src/finish/scallop.rs:952` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop.rs:1034` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/finish/scallop.rs:1125` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop.rs:1132` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop.rs:1140` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop.rs:1160` #[allow(clippy::too_many_arguments, clippy::expect_used)]
- `crates/rs_cam_core/src/finish/scallop.rs:1199` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop.rs:1266` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop.rs:1613` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop.rs:1860` #[allow(clippy::indexing_slicing)] // ring/filtered indexing is guarded by len checks
- `crates/rs_cam_core/src/finish/scallop.rs:1907` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop.rs:1947` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop.rs:1986` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop.rs:2095` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop.rs:2131` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop.rs:2171` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop.rs:2489` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/surface_link.rs:661` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dressup/mod.rs:165` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:753` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:787` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:888` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/mod.rs:923` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/mod.rs:961` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/mod.rs:1015` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dressup/mod.rs:1227` #[allow(clippy::indexing_slicing)] // windows(2) pairs, bounded
- `crates/rs_cam_core/src/dressup/mod.rs:1366` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:1403` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/dressup/mod.rs:1597` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:1672` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:2043` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:2404` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:2954` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/feed_modulation.rs:344` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/feed_modulation.rs:730` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:142` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/tsp.rs:149` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:208` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:310` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:355` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:466` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:533` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:601` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/feedopt.rs:277` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/condition.rs:55` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/arcfit.rs:74` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/arcfit.rs:315` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/arcfit.rs:484` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:167` #[allow(clippy::needless_range_loop, clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:229` #[allow(clippy::needless_range_loop, clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:314` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:487` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:506` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:526` #[allow(clippy::indexing_slicing)]
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
- `crates/rs_cam_core/src/adaptive/material_grid.rs:307` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:318` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/pushcutter.rs:184` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/pushcutter.rs:411` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/pushcutter.rs:422` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/rest_field.rs:561` #[allow(clippy::too_many_arguments)] // one cohesive measurement off five grids
- `crates/rs_cam_core/src/surface/rest_field.rs:777` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/surface/rest_field.rs:1223` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/rest_field.rs:1232` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/rest_field.rs:1341` #[allow(clippy::nonminimal_bool)]
- `crates/rs_cam_core/src/surface/rest_field.rs:1915` #[allow(clippy::indexing_slicing)] // r,c bounded by nx,ny above
- `crates/rs_cam_core/src/surface/rest_field.rs:1935` #[allow(clippy::indexing_slicing)] // all indices bounded by nx/ny by construction
- `crates/rs_cam_core/src/surface/dropcutter.rs:34` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/dropcutter.rs:88` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/dropcutter.rs:498` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/dropcutter.rs:527` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/surface/slope.rs:114` #[allow(clippy::too_many_arguments, clippy::expect_used)]
- `crates/rs_cam_core/src/surface/slope.rs:133` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/surface/slope.rs:236` #[allow(clippy::too_many_arguments, clippy::panic)]
- `crates/rs_cam_core/src/surface/slope.rs:265` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:288` #[allow(clippy::indexing_slicing)] // bounds checked on the line above
- `crates/rs_cam_core/src/surface/slope.rs:329` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:419` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:481` #[allow(clippy::indexing_slicing)] // SAFETY: idx bounded by caller loop ranges
- `crates/rs_cam_core/src/surface/slope.rs:482` #[allow(clippy::needless_pass_by_value)] // tuple of mut refs is the natural pattern
- `crates/rs_cam_core/src/surface/slope.rs:498` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:527` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/slope.rs:559` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/slope.rs:570` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/slope.rs:582` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/slope.rs:592` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/slope.rs:645` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:725` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:732` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:739` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:746` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:753` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/flow_accum.rs:24` //! are therefore bounded by construction; the `#[allow(clippy::indexing_slicing)]`
- `crates/rs_cam_core/src/surface/flow_accum.rs:83` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/flow_accum.rs:133` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/flow_accum.rs:201` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/flow_accum.rs:347` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/flow_accum.rs:383` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:218` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:277` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:318` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:353` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:446` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:455` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:582` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:610` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:642` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:662` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:778` #[allow(clippy::indexing_slicing)] // bounds checked on the line above each index
- `crates/rs_cam_core/src/trace/semantic_trace.rs:911` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:965` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/trace/narrate.rs:898` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/trace/narrate.rs:904` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh.rs:32` #[allow(clippy::indexing_slicing)] // grid indexing bounded by row/col loops
- `crates/rs_cam_core/src/stock/dexel_mesh.rs:192` #[allow(clippy::indexing_slicing)] // grid indexing bounded by row*cols iteration
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:48` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:347` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/stock_mesh.rs:44` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/stock_mesh.rs:102` #[allow(clippy::indexing_slicing)] // stride-3 loop bounded by num_verts
- `crates/rs_cam_core/src/stock/stock_mesh.rs:227` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/stock_mesh.rs:289` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/stock_mesh.rs:416` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/collision.rs:235` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/collision.rs:366` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/collision.rs:466` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel.rs:41` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:62` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:85` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:129` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:163` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:467` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:474` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:481` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:491` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:499` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:506` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:515` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/simulation_cut.rs:1484` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/radial_profile.rs:116` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/radial_profile.rs:118` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/stats.rs:26` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/simulate.rs:1549` #[allow(clippy::too_many_arguments)] // deviation-pass plumbing, mirrors the call site's request fields
- `crates/rs_cam_core/src/compute/simulate.rs:1643` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/simulate.rs:1696` #[allow(clippy::indexing_slicing)] // triangle indices bounded by mesh
- `crates/rs_cam_core/src/compute/execute.rs:3161` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/execute.rs:3366` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/execute.rs:3415` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/execute.rs:3507` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/execute.rs:3688` #[allow(clippy::too_many_arguments)] // post-generation attach point; every
- `crates/rs_cam_core/src/compute/execute.rs:3991` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/execute.rs:4538` #[allow(
- `crates/rs_cam_core/src/compute/execute.rs:4548` #[allow(
- `crates/rs_cam_core/src/compute/execute.rs:4558` #[allow(
- `crates/rs_cam_core/src/compute/execute.rs:4567` #[allow(
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
- `crates/rs_cam_viz/src/render/toolpath_render.rs:126` #[allow(clippy::indexing_slicing)] // n - 1 is safe: n > 0 and n <= len
- `crates/rs_cam_viz/src/render/toolpath_render.rs:153` #[allow(clippy::indexing_slicing)] // loop index i bounded by tp.moves.len()
- `crates/rs_cam_viz/src/render/toolpath_render.rs:450` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/render/toolpath_render.rs:604` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/render/toolpath_render.rs:849` #[allow(clippy::indexing_slicing)] // first_cut_idx validated by position() and bounds > 0
- `crates/rs_cam_viz/src/render/toolpath_render.rs:1006` #[allow(clippy::indexing_slicing)] // first_cut_idx validated by position() and bounds > 0
- `crates/rs_cam_viz/src/render/camera.rs:87` #[allow(clippy::indexing_slicing)] // nalgebra 4x4 matrix slice is always 16 elements
- `crates/rs_cam_viz/src/render/camera.rs:137` #[allow(clippy::indexing_slicing)] // fixed-size [f32; 3] arrays
- `crates/rs_cam_viz/src/state/history.rs:18` #[allow(clippy::large_enum_variant)]
- `crates/rs_cam_viz/src/state/simulation.rs:1224` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_viz/src/state/simulation.rs:1259` #[allow(clippy::indexing_slicing)] // boundary_index from position() is always in bounds
- `crates/rs_cam_viz/src/state/simulation.rs:1412` #[allow(clippy::indexing_slicing)] // child_index from parent's child list, bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:1478` #[allow(clippy::indexing_slicing)] // active_index from active_item_index() bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:2160` #[allow(clippy::indexing_slicing)] // item_index from enumerate(), bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:2339` #[allow(clippy::indexing_slicing)] // item_index from enumerate(), bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:2379` #[allow(clippy::indexing_slicing)] // item indices from move_item_indices, bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:2403` #[allow(clippy::indexing_slicing)] // index from item_index_by_id, bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:2457` #[allow(clippy::indexing_slicing)] // bounds checked: move_end_exclusive <= cumulative.len()-1
- `crates/rs_cam_viz/src/state/simulation.rs:2480` #[allow(clippy::indexing_slicing)] // move_index bounded by caller's loop over toolpath.moves
- `crates/rs_cam_viz/src/ui/sim_debug.rs:92` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:607` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:741` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:810` #[allow(clippy::too_many_arguments, clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs:135` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs:256` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/preflight.rs:317` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:434` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1414` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1492` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:453` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:569` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:674` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:846` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:853` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:866` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1252` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1541` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1653` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1655` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1657` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1659` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1759` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:2150` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:2151` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:2240` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/export_wizard.rs:620` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/feeds/compare.rs:148` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/feeds/compare.rs:258` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/feeds/shared.rs:45` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:1715` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:1977` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:2190` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:2329` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:3111` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:4031` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/setup.rs:35` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/stock.rs:282` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/stock.rs:352` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:109` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:1388` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/io.rs:529` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/toolpath.rs:296` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:408` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:2075` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:2164` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:2186` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/interaction/picking.rs:301` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:52` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:115` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:165` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:292` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:315` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:405` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:519` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:603` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:871` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:966` #[allow(clippy::type_complexity)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:1380` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/viewport.rs:731` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/simulation.rs:34` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:45` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:89` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/simulation.rs:417` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:428` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:490` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/simulation.rs:609` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/mcp.rs:1337` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/app/mcp.rs:4781` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/app/mcp.rs:4988` #[allow(clippy::indexing_slicing)] // pos came from pass_index_of
- `crates/rs_cam_viz/src/app/mcp.rs:5040` #[allow(clippy::indexing_slicing)] // SAFETY: `kind.index()` is bounded by COUNT.
- `crates/rs_cam_viz/src/app/mcp.rs:5192` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/app/mcp/commands.rs:1915` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/compute/worker.rs:3` #[allow(
- `crates/rs_cam_viz/src/compute/worker.rs:12` #[allow(
- `crates/rs_cam_viz/src/compute/worker.rs:20` #[allow(
- `crates/rs_cam_viz/src/compute/worker.rs:310` #[allow(clippy::large_enum_variant)]
- `crates/rs_cam_viz/src/compute/worker.rs:917` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker.rs:1041` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker.rs:1167` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker.rs:1261` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker.rs:1393` #[allow(clippy::expect_used)]
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
- `crates/rs_cam_cli/src/sweep.rs:6` #![allow(clippy::print_stdout)]
- `crates/rs_cam_cli/src/sweep.rs:294` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_cli/src/project.rs:206` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_cli/src/job.rs:183` #[allow(dead_code)]
- `crates/rs_cam_mcp/src/server.rs:1361` #[allow(clippy::needless_pass_by_value)]

## TODO / FIXME / XXX / HACK in production code (10)

- `crates/rs_cam_core/src/feeds/vendor_normalize.rs:269` // minimal practical impact today. TODO Phase E+: extend
- `crates/rs_cam_core/src/feeds/vendor_normalize.rs:305` // TODO Phase 3+: plumb Rockwell HardnessKind through
- `crates/rs_cam_core/src/material/mod.rs:541` /// TODO Phase 3+: replace with a per-species derivation backbone
- `crates/rs_cam_core/src/material/mod.rs:543` /// the `kc.md` TODO trail).
- `crates/rs_cam_core/src/material/mod.rs:844` // TODO deferred), paralleling Phase 2B for sheet goods, which
- `crates/rs_cam_core/src/material/mod.rs:856` // TODO: source from CSIRO or FRI publications.
- `crates/rs_cam_core/src/material/mod.rs:874` // TODO: source from CSIRO publications.
- `crates/rs_cam_core/src/material/mod.rs:877` // retained. TODO: source from EMBRAPA / IPT.
- `crates/rs_cam_core/src/material/mod.rs:888` // TODO Phase 3 — per-grade plywood Kc has no fetched
- `crates/rs_cam_core/src/material/mod.rs:913` // TODO Phase 3: replace with fetched HDF cutting-force
