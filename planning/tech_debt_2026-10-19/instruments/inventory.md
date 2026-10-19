# Inventory (mechanical)

## Largest production files (top 25)

| lines | file |
|---|---|
| 3476 | `crates/rs_cam_viz/src/compute/worker/tests.rs` |
| 3362 | `crates/rs_cam_core/src/compute/simulate.rs` |
| 3344 | `crates/rs_cam_core/src/tool_load/verdict.rs` |
| 3335 | `crates/rs_cam_core/src/adaptive/path.rs` |
| 3303 | `crates/rs_cam_core/src/compute/operation_configs.rs` |
| 3231 | `crates/rs_cam_core/src/gcode/mod.rs` |
| 3201 | `crates/rs_cam_viz/src/app/mcp/commands.rs` |
| 3187 | `crates/rs_cam_core/src/adaptive3d/clearing.rs` |
| 3092 | `crates/rs_cam_core/src/feeds/mod.rs` |
| 3082 | `crates/rs_cam_core/src/dexel_stock/stamping.rs` |
| 3011 | `crates/rs_cam_core/src/polygon.rs` |
| 2982 | `crates/rs_cam_core/src/finish/conformal_spiral.rs` |
| 2975 | `crates/rs_cam_core/src/surface/rest_field.rs` |
| 2858 | `crates/rs_cam_mcp/src/server.rs` |
| 2850 | `crates/rs_cam_core/src/session/command.rs` |
| 2842 | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` |
| 2741 | `crates/rs_cam_core/src/session/compute/tests.rs` |
| 2676 | `crates/rs_cam_core/src/feeds/suggest/tests.rs` |
| 2663 | `crates/rs_cam_core/src/adaptive/mod.rs` |
| 2662 | `crates/rs_cam_viz/src/controller/events/compute.rs` |
| 2646 | `crates/rs_cam_core/src/session/compute.rs` |
| 2640 | `crates/rs_cam_core/src/session/mod.rs` |
| 2637 | `crates/rs_cam_core/src/dressup/mod.rs` |
| 2582 | `crates/rs_cam_core/src/stock/sim_triage.rs` |
| 2551 | `crates/rs_cam_core/src/trace/narrate.rs` |

## Functions >= 250 lines (crude: to the next `fn`) (91)

| lines | fn | file:line |
|---|---|---|
| 1342 | `upload_gpu_data` | `crates/rs_cam_viz/src/app/gpu_upload.rs:155` |
| 1328 | `calculate` | `crates/rs_cam_core/src/feeds/mod.rs:1493` |
| 1086 | `fmt` | `crates/rs_cam_core/src/session/command.rs:986` |
| 910 | `in_simulation` | `crates/rs_cam_viz/src/ui/overlays/registry.rs:489` |
| 902 | `describe_core` | `crates/rs_cam_viz/src/app/mcp/commands.rs:2035` |
| 862 | `adaptive_segments_with_debug` | `crates/rs_cam_core/src/adaptive/path.rs:671` |
| 705 | `from_stock_bbox` | `crates/rs_cam_core/src/feeds/suggest.rs:49` |
| 665 | `scallop_toolpath_research_with_link` | `crates/rs_cam_core/src/finish/scallop/research.rs:215` |
| 648 | `fmt` | `crates/rs_cam_mcp/src/server.rs:78` |
| 642 | `evaluate_inner` | `crates/rs_cam_core/src/tool_load/chipload.rs:371` |
| 642 | `diagnostics_with_evidence` | `crates/rs_cam_core/src/session/compute/diagnostics.rs:921` |
| 594 | `ring_spacing_margin_mm` | `crates/rs_cam_core/src/finish/conformal_spiral.rs:580` |
| 592 | `draw_geometry_tab` | `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:471` |
| 574 | `stacked_simulation_3d_html` | `crates/rs_cam_core/src/export/viz.rs:580` |
| 534 | `adaptive_3d_segments` | `crates/rs_cam_core/src/adaptive3d/path.rs:606` |
| 514 | `new` | `crates/rs_cam_viz/src/render/mod.rs:263` |
| 513 | `entry_for_warning` | `crates/rs_cam_core/src/feeds/rationale.rs:374` |
| 512 | `core_command_for` | `crates/rs_cam_viz/src/app/mcp/commands.rs:332` |
| 509 | `relink_fragments_with_kinds` | `crates/rs_cam_core/src/finish/surface_link.rs:865` |
| 505 | `run_project_command` | `crates/rs_cam_cli/src/project.rs:635` |
| 483 | `draw_frame` | `crates/rs_cam_viz/src/app.rs:860` |
| 481 | `clear_one_region` | `crates/rs_cam_core/src/adaptive3d/clearing.rs:2377` |
| 471 | `apply_dressups` | `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:470` |
| 468 | `from` | `crates/rs_cam_mcp/src/server.rs:726` |
| 465 | `from` | `crates/rs_cam_mcp/src/server.rs:1194` |
| 455 | `prepare` | `crates/rs_cam_viz/src/render/mod.rs:929` |
| 450 | `draw_viewport` | `crates/rs_cam_viz/src/app/viewport.rs:240` |
| 449 | `run_simulation_memoized` | `crates/rs_cam_core/src/compute/simulate.rs:1911` |
| 448 | `handle_events` | `crates/rs_cam_viz/src/app/input.rs:11` |
| 445 | `stamp_swept_chunk` | `crates/rs_cam_core/src/dexel_stock/swept.rs:418` |
| 445 | `draw_toolpath_panel` | `crates/rs_cam_viz/src/ui/properties/toolpath_panel.rs:26` |
| 441 | `update_live_sim` | `crates/rs_cam_viz/src/app/simulation.rs:121` |
| 425 | `resolve_generation_inputs` | `crates/rs_cam_core/src/session/compute/generation.rs:46` |
| 418 | `handle_internal_event` | `crates/rs_cam_viz/src/controller/events/mod.rs:174` |
| 415 | `simulate_toolpath_with_lut_metrics_rapid_checked` | `crates/rs_cam_core/src/dexel_stock/simulation.rs:414` |
| 414 | `stamp_segment_with_metrics` | `crates/rs_cam_core/src/dexel_stock/stamping.rs:1549` |
| 411 | `generate_scallop_rings_with_cancel` | `crates/rs_cam_core/src/finish/scallop/ring_generation.rs:273` |
| 400 | `run_job_command` | `crates/rs_cam_cli/src/job.rs:284` |
| 399 | `handle_mcp_request` | `crates/rs_cam_viz/src/app/mcp.rs:229` |
| 392 | `adopt_simulation_result` | `crates/rs_cam_viz/src/controller/events/compute.rs:675` |
| 390 | `draw_height_diagram` | `crates/rs_cam_viz/src/ui/properties/operations/height_diagram.rs:222` |
| 379 | `draw_chart_c` | `crates/rs_cam_viz/src/ui/feeds/explore.rs:481` |
| 378 | `clear_z_level_contour_parallel` | `crates/rs_cam_core/src/adaptive3d/clearing.rs:1030` |
| 366 | `tool_type` | `crates/rs_cam_core/src/feeds/mod.rs:291` |
| 360 | `apply_lead_in_out` | `crates/rs_cam_core/src/dressup/mod.rs:1449` |
| 358 | `draw_warnings_tab` | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:512` |
| 350 | `generate_setup_sheet_from_session` | `crates/rs_cam_viz/src/io/setup_sheet.rs:69` |
| 348 | `feeds_warning_to_diagnostic` | `crates/rs_cam_core/src/diagnostics/adapters/from_feeds.rs:76` |
| 344 | `ramp_finish_toolpath_structured_annotated_with_resolution` | `crates/rs_cam_core/src/finish/ramp_finish.rs:531` |
| 340 | `apply_adaptive_feed_modulation` | `crates/rs_cam_core/src/session/compute/simulation.rs:486` |
| 334 | `set_toolpath_param_impl` | `crates/rs_cam_core/src/session/compute/params.rs:146` |
| 330 | `apply_entry` | `crates/rs_cam_core/src/dressup/mod.rs:368` |
| 326 | `draw` | `crates/rs_cam_viz/src/ui/preflight.rs:9` |
| 322 | `params` | `crates/rs_cam_core/src/compute/operation_configs.rs:1691` |
| 320 | `span_coverage_cases` | `crates/rs_cam_core/src/compute/execute/tests.rs:346` |
| 320 | `modulate_annotated_against_trace` | `crates/rs_cam_core/src/session/compute.rs:1775` |
| 313 | `apply` | `crates/rs_cam_core/src/session/command.rs:2339` |
| 312 | `model_import_event` | `crates/rs_cam_viz/src/ui/mod.rs:66` |
| 311 | `segments_to_toolpath` | `crates/rs_cam_core/src/adaptive3d/path.rs:1382` |
| 309 | `try_fit_arc` | `crates/rs_cam_core/src/dressup/arcfit.rs:384` |

## `allow(` in production code (693)

- `crates/rs_cam_cli/src/rough_score.rs:1` #![allow(clippy::print_stdout)] // CLI surface: the JSON records go to stdout
- `crates/rs_cam_cli/src/nc_replay.rs:1` #![allow(clippy::print_stdout)] // CLI surface
- `crates/rs_cam_cli/src/nc_replay.rs:311` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_cli/src/run.rs:31` #[allow(clippy::struct_excessive_bools)]
- `crates/rs_cam_cli/src/smoke.rs:14` #![allow(clippy::print_stdout)] // CLI surface
- `crates/rs_cam_cli/src/smoke.rs:448` #[allow(clippy::result_large_err)] // tp_idx tuple is small; failure path is the rare branch
- `crates/rs_cam_cli/src/main.rs:2` #![allow(clippy::print_stderr)] // CLI uses eprintln! for user-facing diagnostic output
- `crates/rs_cam_cli/src/main.rs:3` #![allow(clippy::print_stdout)] // CLI `version` prints build info to stdout
- `crates/rs_cam_cli/src/sweep.rs:6` #![allow(clippy::print_stdout)]
- `crates/rs_cam_cli/src/sweep.rs:331` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_cli/src/project.rs:634` #[allow(clippy::too_many_arguments)]
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
- `crates/rs_cam_core/src/polygon.rs:1616` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1655` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1699` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1711` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1779` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1787` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1809` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1823` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/polygon.rs:1831` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1926` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/polygon.rs:1965` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/interrupt.rs:35` /// `#[allow(clippy::expect_used)]` over the `Result` it made unreachable.
- `crates/rs_cam_core/src/interrupt.rs:71` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/toolpath.rs:205` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:221` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:247` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:283` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:338` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:502` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:600` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/toolpath.rs:778` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)] // bounded by grid dimensions
- `crates/rs_cam_core/src/toolpath.rs:872` #[allow(clippy::indexing_slicing)] // first element access guarded by is_empty check
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
- `crates/rs_cam_core/src/geo.rs:416` #[allow(clippy::unwrap_used, clippy::panic)] // Tests: unwrap is idiomatic for asserting success
- `crates/rs_cam_core/src/diagnostics/tests.rs:5` #![allow(
- `crates/rs_cam_core/src/diagnostics/adapters/from_feeds.rs:511` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/compute.rs:1774` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/compute.rs:1900` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:1918` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:1924` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:1927` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:1930` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/compute.rs:2494` // rather than `#[allow(clippy::unwrap_used)]`.
- `crates/rs_cam_core/src/session/compute.rs:2583` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/mutation/toolpath.rs:572` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/session/mutation/tests.rs:5` #![allow(
- `crates/rs_cam_core/src/session/compute/params.rs:122` #[allow(dead_code)]
- `crates/rs_cam_core/src/session/compute/params.rs:178` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_core/src/session/compute/params.rs:276` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/session/compute/simulation.rs:353` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/session/compute/simulation.rs:364` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/compute/tests.rs:4` #![allow(
- `crates/rs_cam_core/src/session/compute/generation.rs:827` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/session/compute/generation.rs:1023` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/io/dxf_input.rs:373` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/io/dxf_input.rs:395` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/io/dxf_input.rs:603` #[allow(clippy::indexing_slicing)] // len >= 2 guarded below
- `crates/rs_cam_core/src/io/dxf_input.rs:646` #[allow(clippy::indexing_slicing)] // piece indices come from 0..pieces.len()
- `crates/rs_cam_core/src/io/dxf_input.rs:763` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/io/dxf_input.rs:820` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/io/svg_input.rs:174` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/io/svg_input.rs:177` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/io/step_input.rs:46` #[allow(clippy::indexing_slicing)] // mesh vertex/face indices bounded by tessellation output
- `crates/rs_cam_core/src/io/step_input.rs:247` #[allow(clippy::indexing_slicing)] // triangle indices [0..3] and vertex lookups bounded by mesh
- `crates/rs_cam_core/src/io/step_input.rs:354` #[allow(clippy::indexing_slicing)] // vertex indices bounded by mesh topology
- `crates/rs_cam_core/src/io/step_input.rs:417` #[allow(clippy::indexing_slicing)] // vertex indices bounded by face tessellation
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:139` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:278` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:284` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/enriched_mesh.rs:438` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/marching_squares.rs:149` #[allow(clippy::indexing_slicing)] // SAFETY: indices bounded by segment count
- `crates/rs_cam_core/src/geometry/grid_field.rs:21` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by loop variables and n
- `crates/rs_cam_core/src/geometry/grid_field.rs:78` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by rows/cols loop variables
- `crates/rs_cam_core/src/geometry/grid_field.rs:127` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by row/col loop ranges
- `crates/rs_cam_core/src/geometry/grid_field.rs:180` #[allow(clippy::indexing_slicing)] // SAFETY: all indices bounded by row/col loop ranges
- `crates/rs_cam_core/src/geometry/edge_distance.rs:85` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/edge_distance.rs:288` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/geometry/fiber.rs:134` #[allow(clippy::indexing_slicing)] // SAFETY: indices bounded by len checks
- `crates/rs_cam_core/src/geometry/boundary.rs:700` #[allow(clippy::indexing_slicing)] // bounded by grid dimensions computed from mesh bbox
- `crates/rs_cam_core/src/geometry/boundary.rs:771` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/boundary.rs:843` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/geometry/contour_extract.rs:65` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/geometry/contour_extract.rs:94` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/geometry/contour_extract.rs:148` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/geometry/contour_extract.rs:170` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/geometry/contour_extract.rs:268` #[allow(clippy::indexing_slicing)] // SAFETY: row/col bounded by loop ranges checked above
- `crates/rs_cam_core/src/geometry/grid2.rs:26` //! raw slice indexing anywhere in this module, so no `#[allow(clippy::indexing_slicing)]`
- `crates/rs_cam_core/src/geometry/arc_util.rs:81` #[allow(clippy::unwrap_used, clippy::panic)] // Tests: unwrap is idiomatic for asserting success
- `crates/rs_cam_core/src/dexel_stock/band.rs:83` #[allow(clippy::indexing_slicing)] // caller derives `local` from clamped bounds
- `crates/rs_cam_core/src/dexel_stock/sample_coalesce.rs:201` #[allow(clippy::indexing_slicing)] // first < end <= samples.len(), by the caller
- `crates/rs_cam_core/src/dexel_stock/mod.rs:624` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/mod.rs:639` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/swept.rs:196` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:249` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:250` #[allow(clippy::indexing_slicing)] // bounded by clamped bbox / bin count
- `crates/rs_cam_core/src/dexel_stock/swept.rs:271` #[allow(clippy::needless_range_loop)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:416` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:417` #[allow(clippy::indexing_slicing)] // bounded by clamped bbox / bin count
- `crates/rs_cam_core/src/dexel_stock/swept.rs:651` #[allow(clippy::needless_range_loop)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:708` #[allow(clippy::needless_range_loop)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:724` #[allow(clippy::needless_range_loop)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:773` #[allow(clippy::needless_range_loop)]
- `crates/rs_cam_core/src/dexel_stock/swept.rs:990` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/whole_path.rs:324` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:219` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:560` #[allow(clippy::indexing_slicing)] // caller derives idx from clamped bounds
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:565` #[allow(clippy::indexing_slicing)] // caller derives idx from clamped bounds
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:589` #[allow(clippy::indexing_slicing)] // caller derives idx from clamped bounds
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:594` #[allow(clippy::indexing_slicing)] // caller derives idx from clamped bounds
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:749` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:760` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:858` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:866` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:1030` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:1031` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:1502` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:1503` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/stamping.rs:2038` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/tile_mip.rs:194` #[allow(clippy::indexing_slicing)] // bounded by rows/cols, checked above
- `crates/rs_cam_core/src/dexel_stock/tile_mip.rs:218` #[allow(clippy::indexing_slicing)] // tile indices derived from clamped cells
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:125` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:126` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:271` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:322` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:360` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:412` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:413` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:828` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:928` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1089` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1296` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1331` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1377` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1521` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1557` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dexel_stock/simulation.rs:1769` #[allow(clippy::indexing_slicing)] // bounded by the loop range
- `crates/rs_cam_core/src/ops/adaptive_shared.rs:115` #[allow(clippy::indexing_slicing, clippy::expect_used)]
- `crates/rs_cam_core/src/ops/adaptive_shared.rs:121` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/adaptive_shared.rs:124` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/zigzag.rs:34` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/ops/zigzag.rs:95` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/ops/project_curve.rs:147` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/project_curve.rs:164` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/project_curve.rs:167` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/ops/project_curve.rs:202` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/ops/project_curve.rs:205` #[allow(clippy::cast_precision_loss)] // SAFETY: k < steps, a small count
- `crates/rs_cam_core/src/ops/project_curve.rs:260` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/ops/project_curve.rs:307` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/ops/pocket.rs:144` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/ops/face.rs:154` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/face.rs:157` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/face.rs:165` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/waterline.rs:242` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/ops/inlay.rs:186` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/inlay.rs:265` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/ops/vcarve.rs:70` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/ops/vcarve.rs:83` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/mod.rs:535` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive3d/mod.rs:617` #[allow(clippy::type_complexity)]
- `crates/rs_cam_core/src/adaptive3d/centre_clip.rs:120` #[allow(clippy::indexing_slicing)] // SAFETY: indices bounded by rows/cols checks
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:102` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:418` #[allow(clippy::indexing_slicing)] // SAFETY: padded grid indices bounded by loop ranges
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:598` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:888` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:933` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:962` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1029` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1169` #[allow(clippy::indexing_slicing)] // idx < path_3d.len() by construction
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1296` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1407` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1573` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1579` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1759` #[allow(clippy::indexing_slicing)] // anchors indexed by enumerate idx
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1794` #[allow(clippy::indexing_slicing)] // SAFETY: i < n, (i+1) % n < n
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1796` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:1993` #[allow(clippy::indexing_slicing)] // order entries are valid region indices
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:2235` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:2376` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/clearing.rs:2883` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/path.rs:34` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive3d/path.rs:379` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/path.rs:572` #[allow(clippy::type_complexity)] // diagnostic probe; returns raw cell-grid metadata
- `crates/rs_cam_core/src/adaptive3d/path.rs:605` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/path.rs:1213` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive3d/search.rs:45` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/search.rs:182` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive3d/tests.rs:3` #![allow(
- `crates/rs_cam_core/src/export/ribbon.rs:46` #[allow(clippy::indexing_slicing)] // stride-3 loop bounded by num_verts
- `crates/rs_cam_core/src/export/ribbon.rs:189` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/ribbon.rs:251` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/ribbon.rs:378` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/viz.rs:50` #[allow(clippy::indexing_slicing)] // i starts at 1, so i-1 is always valid
- `crates/rs_cam_core/src/export/viz.rs:230` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/viz.rs:281` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/export/viz.rs:365` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/export/viz.rs:574` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/export/viz.rs:1159` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/viz.rs:1174` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/viz.rs:1200` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/viz.rs:1216` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/export/fingerprint.rs:1122` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1169` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1309` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1382` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1491` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1516` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/export/fingerprint.rs:1564` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/machine/mod.rs:596` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine/strategy_advisor.rs:171` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/machine/kinematics.rs:813` #[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
- `crates/rs_cam_core/src/machine/kinematics.rs:1015` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine/kinematics.rs:1074` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/machine/kinematics.rs:1471` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/tool_load/chipload.rs:634` #[allow(clippy::indexing_slicing)] // SAFETY: non-empty checked above
- `crates/rs_cam_core/src/tool_load/chipload.rs:825` #[allow(clippy::indexing_slicing)] // SAFETY: median_idx < len() by construction
- `crates/rs_cam_core/src/feeds/vendor_lut.rs:671` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/feeds/tests.rs:4` #![allow(clippy::unwrap_used, clippy::panic)]
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:325` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:350` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:776` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feeds/vendor_lookup.rs:795` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/feeds/suggest/apply.rs:96` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest/apply.rs:389` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest/apply.rs:420` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest/apply.rs:451` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest/apply.rs:636` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest/invariants.rs:164` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:4` #![allow(
- `crates/rs_cam_core/src/feeds/suggest/tests.rs:2535` #[allow(clippy::match_same_arms)] // one arm per op = the point
- `crates/rs_cam_core/src/feeds/suggest/aggressiveness.rs:456` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/ramp_finish.rs:258` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/ramp_finish.rs:287` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/ramp_finish.rs:320` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/ramp_finish.rs:500` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/ramp_finish.rs:530` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/direction_field.rs:502` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/direction_field.rs:648` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/direction_field.rs:1087` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/direction_field.rs:1152` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/direction_field.rs:1352` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/direction_field.rs:1531` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/spiral_finish.rs:145` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/unified_finish.rs:1426` #[allow(clippy::too_many_arguments)] // op-generator adapter surface, mirrors the strategy fns it composes
- `crates/rs_cam_core/src/finish/unified_finish.rs:1743` #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:136` #[allow(clippy::indexing_slicing)] // SAFETY: i = row*cols + col with both bounded by the loops
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:196` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:242` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:305` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:371` #[allow(clippy::indexing_slicing)] // SAFETY: bounds checked before each read
- `crates/rs_cam_core/src/finish/scallop_isofield.rs:427` #[allow(clippy::indexing_slicing)] // SAFETY: row/col bounded by the loop ranges
- `crates/rs_cam_core/src/finish/finish_planner.rs:1191` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/crease_paths.rs:97` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/classify_probe.rs:350` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/classify_probe.rs:361` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/classify_probe.rs:403` #[allow(clippy::indexing_slicing)] // SAFETY: every index is derived from the tile bounds below
- `crates/rs_cam_core/src/finish/classify_probe.rs:540` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/classify_probe.rs:560` #[allow(clippy::indexing_slicing)] // SAFETY: tile bounds are derived from spec.rows/cols
- `crates/rs_cam_core/src/finish/steep_shallow.rs:90` #[allow(clippy::indexing_slicing)] // bounded indexing in grid morphology
- `crates/rs_cam_core/src/finish/steep_shallow.rs:154` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/steep_shallow.rs:374` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/pencil.rs:397` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/pencil.rs:412` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/pencil.rs:537` #[allow(clippy::indexing_slicing)] // i < n by loop guard
- `crates/rs_cam_core/src/finish/pencil.rs:555` #[allow(clippy::indexing_slicing)] // depths non-empty → index < len
- `crates/rs_cam_core/src/finish/crest_lines.rs:226` #[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
- `crates/rs_cam_core/src/finish/crest_lines.rs:281` #[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
- `crates/rs_cam_core/src/finish/crest_lines.rs:324` #[allow(clippy::indexing_slicing)] // fixed 4×4 literal indices
- `crates/rs_cam_core/src/finish/crest_lines.rs:333` #[allow(clippy::indexing_slicing)] // all indices are mesh vertex/face indices or fixed 0..3
- `crates/rs_cam_core/src/finish/crest_lines.rs:573` #[allow(clippy::indexing_slicing)] // tri vertex indices validated on mesh load
- `crates/rs_cam_core/src/finish/crest_lines.rs:606` #[allow(clippy::indexing_slicing)] // tri/loop indices bounded to mesh data and 0..3
- `crates/rs_cam_core/src/finish/horizontal_finish.rs:73` #[allow(clippy::indexing_slicing, clippy::expect_used)]
- `crates/rs_cam_core/src/finish/horizontal_finish.rs:96` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)] // mesh vertex/face indexing is bounded by mesh structure
- `crates/rs_cam_core/src/finish/horizontal_finish.rs:306` #[allow(clippy::indexing_slicing)] // tri_idx bounded by flat_face_set.len() check
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:1187` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:1285` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:1530` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:2551` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:2669` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/conformal_spiral.rs:2781` #[allow(clippy::result_large_err)]
- `crates/rs_cam_core/src/finish/pencil_dihedral.rs:54` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/pencil_dihedral.rs:72` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/pencil_dihedral.rs:139` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/pencil_dihedral.rs:346` #[allow(clippy::indexing_slicing)] // chain entries are valid vertex indices
- `crates/rs_cam_core/src/finish/scallop.rs:284` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop.rs:286` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop.rs:336` #[allow(clippy::indexing_slicing)] // SAFETY: caller guarantees non-empty
- `crates/rs_cam_core/src/finish/scallop.rs:1022` #[allow(clippy::indexing_slicing)] // ring/filtered indexing is guarded by len checks
- `crates/rs_cam_core/src/finish/scallop.rs:1059` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop.rs:1098` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop.rs:1137` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/surface_link.rs:35` #[allow(clippy::too_many_arguments)] // SAFETY: the link's geometry dials, each named at the call site
- `crates/rs_cam_core/src/finish/surface_link.rs:327` #[allow(clippy::too_many_arguments)] // SAFETY: the candidate, the surface and the ceiling are separate inputs
- `crates/rs_cam_core/src/finish/surface_link.rs:864` #[allow(clippy::too_many_arguments)] // SAFETY: the relink inputs plus the fragment kinds, which ride a parameter (see a
- `crates/rs_cam_core/src/finish/scallop/ring_generation.rs:165` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop/ring_generation.rs:198` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop/ring_generation.rs:205` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop/ring_generation.rs:213` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop/ring_generation.rs:233` #[allow(clippy::too_many_arguments, clippy::expect_used)]
- `crates/rs_cam_core/src/finish/scallop/ring_generation.rs:272` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop/ring_generation.rs:353` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop/ring_generation.rs:707` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop/research.rs:83` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop/research.rs:126` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop/research.rs:162` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop/research.rs:214` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/scallop/research.rs:532` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/scallop/tests.rs:4` #![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
- `crates/rs_cam_core/src/finish/unified_finish/region_routing.rs:76` #[allow(clippy::too_many_arguments)] // lattice dials (step, direction, window) ride beside the op params, mirroring the
- `crates/rs_cam_core/src/finish/unified_finish/region_routing.rs:339` #[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
- `crates/rs_cam_core/src/finish/unified_finish/region_routing.rs:426` #[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
- `crates/rs_cam_core/src/finish/unified_finish/region_routing.rs:604` #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
- `crates/rs_cam_core/src/finish/unified_finish/tests.rs:4` #![allow(
- `crates/rs_cam_core/src/finish/conformal_spiral/spiral_build.rs:173` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/conformal_spiral/tests.rs:4` #![allow(
- `crates/rs_cam_core/src/finish/pencil/detectors.rs:200` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/finish/pencil/detectors.rs:357` #[allow(clippy::indexing_slicing)] // face_a/face_b are valid mesh face indices
- `crates/rs_cam_core/src/finish/pencil/chain_paths.rs:18` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/finish/pencil/chain_paths.rs:94` #[allow(clippy::indexing_slicing)] // i bounded to 1..len-1; neighbours i±1 valid
- `crates/rs_cam_core/src/finish/pencil/chain_paths.rs:267` #[allow(clippy::too_many_arguments)] // cohesive per-chain emit; splitting hurts clarity
- `crates/rs_cam_core/src/finish/pencil/emission.rs:503` #[allow(clippy::too_many_arguments)] // SAFETY: the link end points, the stock, the ceiling and the contact dials are se
- `crates/rs_cam_core/src/finish/pencil/emission.rs:631` #[allow(clippy::too_many_arguments)] // SAFETY: the emitter inputs plus the two stock readings
- `crates/rs_cam_core/src/finish/pencil/emission.rs:702` #[allow(clippy::too_many_arguments)] // SAFETY: the emitter inputs plus the two stock readings
- `crates/rs_cam_core/src/finish/pencil/tests.rs:4` #![allow(clippy::unwrap_used, clippy::panic)]
- `crates/rs_cam_core/src/finish/pencil/tests.rs:310` #[allow(clippy::indexing_slicing)] // bounded by moves.len()
- `crates/rs_cam_core/src/finish/pencil/tests.rs:1173` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/dressup/mod.rs:367` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:1048` #[allow(clippy::indexing_slicing)] // i + 1 < moves.len() checked first
- `crates/rs_cam_core/src/dressup/mod.rs:1060` #[allow(clippy::indexing_slicing)] // i, j < moves.len() by loop bounds
- `crates/rs_cam_core/src/dressup/mod.rs:1110` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:1162` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/dressup/mod.rs:1448` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:1808` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:1990` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/mod.rs:2356` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/feed_modulation.rs:401` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/feed_modulation.rs:892` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:148` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:205` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:318` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:404` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:420` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:450` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:481` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:521` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:609` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tsp.rs:676` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/feedopt.rs:273` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/tests.rs:4` #![allow(
- `crates/rs_cam_core/src/dressup/entry_descent.rs:91` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/entry_descent.rs:125` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/entry_descent.rs:254` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/entry_descent.rs:289` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/entry_descent.rs:330` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/entry_descent.rs:481` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/dressup/entry_descent.rs:695` #[allow(clippy::indexing_slicing)] // windows(2) pairs, bounded
- `crates/rs_cam_core/src/dressup/condition.rs:50` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/arcfit.rs:89` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/arcfit.rs:376` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/dressup/arcfit.rs:545` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/dressup/arcfit.rs:664` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:167` #[allow(clippy::needless_range_loop, clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:229` #[allow(clippy::needless_range_loop, clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:314` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:487` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:506` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/metrology/monge.rs:526` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive/path.rs:216` #[allow(clippy::indexing_slicing)] // bounded lattice scan
- `crates/rs_cam_core/src/adaptive/path.rs:418` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive/path.rs:669` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:1324` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive/path.rs:1532` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:1620` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:1762` #[allow(clippy::indexing_slicing)] // i < filtered.len()
- `crates/rs_cam_core/src/adaptive/path.rs:1793` #[allow(clippy::indexing_slicing)] // machinable_vec non-empty checked above
- `crates/rs_cam_core/src/adaptive/path.rs:1938` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:2390` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive/path.rs:2478` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:2642` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive/path.rs:2736` #[allow(clippy::indexing_slicing)] // path.len() >= 2 checked above
- `crates/rs_cam_core/src/adaptive/path.rs:2738` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/adaptive/path.rs:2800` #[allow(clippy::indexing_slicing)] // i < contour.len() bounded above
- `crates/rs_cam_core/src/adaptive/path.rs:2822` #[allow(clippy::indexing_slicing)] // len >= 2 checked
- `crates/rs_cam_core/src/adaptive/path.rs:2827` #[allow(clippy::indexing_slicing)] // len >= 2 checked
- `crates/rs_cam_core/src/adaptive/path.rs:2846` #[allow(clippy::indexing_slicing)] // best_idx in 0..verts.len()
- `crates/rs_cam_core/src/adaptive/path.rs:2848` #[allow(clippy::indexing_slicing)] // best_idx in 0..verts.len()
- `crates/rs_cam_core/src/adaptive/path.rs:2875` #[allow(clippy::indexing_slicing)] // contour.len() >= 2 checked above
- `crates/rs_cam_core/src/adaptive/path.rs:2881` #[allow(clippy::indexing_slicing)] // i, (i+1)%n bounded by contour len
- `crates/rs_cam_core/src/adaptive/path.rs:2883` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive/path.rs:2900` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/path.rs:2998` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive/path.rs:3001` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive/spiral.rs:59` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/adaptive/spiral.rs:60` #[allow(clippy::indexing_slicing)] // bounded indexing over grid/loop buffers
- `crates/rs_cam_core/src/adaptive/spiral.rs:279` #[allow(clippy::indexing_slicing)] // fixed-size direction/loop sampling
- `crates/rs_cam_core/src/adaptive/spiral.rs:330` #[allow(clippy::indexing_slicing)] // padded-grid indices bounded by construction
- `crates/rs_cam_core/src/adaptive/spiral.rs:371` #[allow(clippy::indexing_slicing)] // loop indices bounded by len
- `crates/rs_cam_core/src/adaptive/search.rs:104` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:52` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:99` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:136` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:176` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:253` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:332` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:422` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:454` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:467` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/adaptive/material_grid.rs:538` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:608` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/adaptive/material_grid.rs:619` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/budget/estimate.rs:521` #[allow(clippy::panic)]
- `crates/rs_cam_core/src/budget/tests.rs:2` #![allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/surface/pushcutter.rs:211` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/pushcutter.rs:605` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/pushcutter.rs:616` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/rest_field.rs:559` #[allow(clippy::too_many_arguments)] // one cohesive measurement off five grids
- `crates/rs_cam_core/src/surface/rest_field.rs:815` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/surface/rest_field.rs:1366` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/rest_field.rs:1375` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/rest_field.rs:1484` #[allow(clippy::nonminimal_bool)]
- `crates/rs_cam_core/src/surface/rest_field.rs:2058` #[allow(clippy::indexing_slicing)] // r,c bounded by nx,ny above
- `crates/rs_cam_core/src/surface/rest_field.rs:2078` #[allow(clippy::indexing_slicing)] // all indices bounded by nx/ny by construction
- `crates/rs_cam_core/src/surface/dropcutter.rs:34` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/dropcutter.rs:88` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/dropcutter.rs:498` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/dropcutter.rs:563` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/surface/stock_reference.rs:95` #[allow(clippy::panic)] // SAFETY: a test asserts the variant by panicking on the wrong one
- `crates/rs_cam_core/src/surface/slope.rs:116` #[allow(clippy::too_many_arguments, clippy::expect_used)]
- `crates/rs_cam_core/src/surface/slope.rs:135` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/surface/slope.rs:238` #[allow(clippy::too_many_arguments, clippy::panic)]
- `crates/rs_cam_core/src/surface/slope.rs:267` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:290` #[allow(clippy::indexing_slicing)] // bounds checked on the line above
- `crates/rs_cam_core/src/surface/slope.rs:331` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:421` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:483` #[allow(clippy::indexing_slicing)] // SAFETY: idx bounded by caller loop ranges
- `crates/rs_cam_core/src/surface/slope.rs:484` #[allow(clippy::needless_pass_by_value)] // tuple of mut refs is the natural pattern
- `crates/rs_cam_core/src/surface/slope.rs:500` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:529` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/slope.rs:561` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/slope.rs:572` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/slope.rs:584` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/slope.rs:594` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/slope.rs:647` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:727` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:734` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:741` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:748` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/slope.rs:755` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/surface/flow_accum.rs:24` //! are therefore bounded by construction; the `#[allow(clippy::indexing_slicing)]`
- `crates/rs_cam_core/src/surface/flow_accum.rs:83` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/flow_accum.rs:133` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/flow_accum.rs:201` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/flow_accum.rs:347` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/flow_accum.rs:383` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/chord_refine.rs:109` #[allow(clippy::too_many_arguments)] // SAFETY: the ctx fields, named at the call site
- `crates/rs_cam_core/src/surface/chord_refine.rs:351` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_core/src/surface/merge_tree.rs:170` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/surface/merge_tree.rs:199` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:218` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:277` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:318` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:353` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:446` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/debug_trace.rs:455` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:666` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:694` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:726` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:746` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:862` #[allow(clippy::indexing_slicing)] // bounds checked on the line above each index
- `crates/rs_cam_core/src/trace/semantic_trace.rs:995` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/trace/semantic_trace.rs:1021` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/trace/narrate.rs:932` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/trace/narrate.rs:938` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh.rs:56` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh.rs:300` #[allow(clippy::indexing_slicing)] // grid indexing bounded by row*cols iteration
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:62` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:227` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:454` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:528` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:602` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:835` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh_mc.rs:891` #[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
- `crates/rs_cam_core/src/stock/stock_mesh.rs:90` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/collision.rs:428` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/collision.rs:564` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel.rs:53` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:74` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:97` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:141` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:175` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:209` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:260` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:610` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:617` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:624` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:634` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:711` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:718` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:727` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/dexel.rs:774` #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
- `crates/rs_cam_core/src/stock/radial_profile.rs:116` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/radial_profile.rs:118` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:17` #![allow(clippy::indexing_slicing, clippy::panic)]
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:32` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/dexel_mesh_mc/stride_tests.rs:335` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/simulation_cut/accumulate.rs:149` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/stock/simulation_cut/tests.rs:3` #![allow(
- `crates/rs_cam_core/src/settings/tests.rs:2` #![allow(clippy::unwrap_used)]
- `crates/rs_cam_core/src/compute/stats.rs:26` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/simulate.rs:1498` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/simulate.rs:1575` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/simulate.rs:2533` #[allow(clippy::too_many_arguments)] // deviation-pass plumbing, mirrors the call site's request fields
- `crates/rs_cam_core/src/compute/simulate.rs:2627` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/simulate.rs:2680` #[allow(clippy::indexing_slicing)] // triangle indices bounded by mesh
- `crates/rs_cam_core/src/compute/execute.rs:638` #[allow(
- `crates/rs_cam_core/src/compute/execute.rs:648` #[allow(
- `crates/rs_cam_core/src/compute/execute.rs:658` #[allow(
- `crates/rs_cam_core/src/compute/execute.rs:667` #[allow(
- `crates/rs_cam_core/src/compute/alignment_pins/placement.rs:389` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_core/src/compute/catalog/tests.rs:3` #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
- `crates/rs_cam_core/src/compute/catalog/tests.rs:901` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_core/src/compute/execute/drilling.rs:49` #[allow(clippy::too_many_arguments)] // SAFETY: each argument is one generator input this view must mirror
- `crates/rs_cam_core/src/compute/execute/tests.rs:4` #![allow(
- `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:58` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_core/src/compute/execute/dressup_apply.rs:425` /// carried its own `#[allow(clippy::too_many_arguments)]`. Nine of them are
- `crates/rs_cam_core/src/selection/tests.rs:6` #![allow(
- `crates/rs_cam_core/src/material/wood_species_library.rs:70` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/gcode/post.rs:459` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/gcode/post.rs:468` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/gcode/post.rs:477` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/gcode/post.rs:486` #[allow(clippy::expect_used)]
- `crates/rs_cam_core/src/gcode/emitter.rs:414` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_mcp/src/server.rs:1959` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_viz/src/mcp_server.rs:498` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_viz/src/mcp_server.rs:568` #[allow(clippy::needless_pass_by_value)] Parameters(CutTraceParam {
- `crates/rs_cam_viz/src/mcp_server.rs:614` #[allow(clippy::needless_pass_by_value)] Parameters(GenDebugTraceParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1021` #[allow(clippy::needless_pass_by_value)] Parameters(IndexParam { index }): Parameters<
- `crates/rs_cam_viz/src/mcp_server.rs:1062` #[allow(clippy::needless_pass_by_value)] Parameters(AddToolpathViaGuiParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1082` #[allow(clippy::needless_pass_by_value)] Parameters(GetNotificationsParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1226` #[allow(clippy::needless_pass_by_value)] Parameters(spec): Parameters<
- `crates/rs_cam_viz/src/mcp_server.rs:1242` #[allow(clippy::needless_pass_by_value)] Parameters(spec): Parameters<PreviewTierMapParam>,
- `crates/rs_cam_viz/src/mcp_server.rs:1374` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_viz/src/mcp_server.rs:1394` #[allow(clippy::needless_pass_by_value)]
- `crates/rs_cam_viz/src/mcp_server.rs:1418` #[allow(clippy::needless_pass_by_value)] Parameters(GenerateAllParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1645` #[allow(clippy::needless_pass_by_value)] Parameters(ScreenshotSimParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1673` #[allow(clippy::needless_pass_by_value)] Parameters(ScreenshotToolpathParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1729` #[allow(clippy::needless_pass_by_value)] Parameters(ScreenshotGuiParam {
- `crates/rs_cam_viz/src/mcp_server.rs:1753` #[allow(clippy::needless_pass_by_value)] Parameters(SetUiViewParam {
- `crates/rs_cam_viz/src/controller.rs:8` #[allow(
- `crates/rs_cam_viz/src/controller.rs:16` #[allow(
- `crates/rs_cam_viz/src/controller.rs:25` #[allow(
- `crates/rs_cam_viz/src/controller.rs:34` #[allow(
- `crates/rs_cam_viz/src/controller.rs:42` #[allow(
- `crates/rs_cam_viz/src/render/sim_render.rs:81` #[allow(clippy::indexing_slicing)] // stride loop bounded by colors.len()
- `crates/rs_cam_viz/src/render/sim_render.rs:163` #[allow(clippy::indexing_slicing)] // stride-3 loop bounded by num_verts = vertices.len()/3
- `crates/rs_cam_viz/src/render/sim_render.rs:256` #[allow(clippy::indexing_slicing)] // triangle stride-3 access bounded by loop
- `crates/rs_cam_viz/src/render/sim_render.rs:345` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/render/sim_render.rs:390` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/render/sim_render.rs:408` #[allow(clippy::indexing_slicing)] // stride-3 loops bounded by vertex/index counts
- `crates/rs_cam_viz/src/render/mod.rs:938` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/render/mod.rs:1047` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/render/mod.rs:1050` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/render/mod.rs:1392` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/render/colors.rs:21` #[allow(clippy::indexing_slicing)] // modulo indexing into constant-length palette
- `crates/rs_cam_viz/src/render/fixture_render.rs:138` #[allow(clippy::indexing_slicing)]
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
- `crates/rs_cam_viz/src/state/history.rs:19` #[allow(clippy::large_enum_variant)]
- `crates/rs_cam_viz/src/state/simulation.rs:1389` #[allow(clippy::indexing_slicing)] // item_index from enumerate(), bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:1429` #[allow(clippy::indexing_slicing)] // item indices from move_item_indices, bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation.rs:1453` #[allow(clippy::indexing_slicing)] // index from item_index_by_id, bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation/semantic_trace.rs:52` #[allow(clippy::indexing_slicing)] // active_index from active_item_index() bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation/semantic_trace.rs:305` #[allow(clippy::indexing_slicing)] // item_index from enumerate(), bounded by trace.items
- `crates/rs_cam_viz/src/state/simulation/tests.rs:3` #![allow(
- `crates/rs_cam_viz/src/state/simulation/playback_state.rs:234` #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
- `crates/rs_cam_viz/src/ui/sim_debug.rs:92` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:716` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:848` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_op_list.rs:917` #[allow(clippy::too_many_arguments, clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs:385` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs:472` #[allow(clippy::cast_precision_loss)]
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs:585` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs:817` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs:823` #[allow(clippy::cast_precision_loss)]
- `crates/rs_cam_viz/src/ui/preflight.rs:374` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:2384` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:402` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:694` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:806` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:808` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:810` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:812` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:912` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1309` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1310` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/sim_timeline.rs:1398` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/export_wizard.rs:629` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/components/histogram.rs:382` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/components/histogram.rs:699` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/feeds/compare.rs:157` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/feeds/shared.rs:45` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/feeds/explore.rs:480` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/feeds/explore.rs:1207` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/machine_panel.rs:114` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/machine_panel.rs:375` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/feeds_speeds.rs:20` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/feeds_speeds.rs:164` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/mod.rs:170` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/tests.rs:3` #![allow(
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:122` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/operations/shape_diagrams.rs:1057` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/ui/properties/operations/validate.rs:417` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/ui/properties/pins/plan_view.rs:72` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_viz/src/ui/properties/pins/plan_view.rs:87` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_viz/src/ui/properties/pins/plan_view.rs:109` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_viz/src/ui/properties/pins/plan_view.rs:122` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_viz/src/ui/properties/pins/plan_view.rs:325` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_viz/src/ui/properties/pins/plan_view.rs:348` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_viz/src/ui/properties/pins/flip_view.rs:177` #[allow(clippy::cast_possible_truncation)]
- `crates/rs_cam_viz/src/controller/io.rs:538` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/toolpath.rs:302` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:2411` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:2538` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/controller/events/compute.rs:2560` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/interaction/picking.rs:75` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/interaction/picking.rs:356` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:112` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:162` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:306` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:329` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:439` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:595` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:891` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:986` #[allow(clippy::type_complexity)]
- `crates/rs_cam_viz/src/app/gpu_upload.rs:1443` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/viewport.rs:844` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/simulation.rs:55` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:66` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:120` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/simulation.rs:513` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:524` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/simulation.rs:602` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/simulation.rs:737` #[allow(clippy::unwrap_used)]
- `crates/rs_cam_viz/src/app/mcp.rs:1067` #[allow(
- `crates/rs_cam_viz/src/app/mcp/diagnostics.rs:818` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/app/mcp/commands.rs:2155` #[allow(clippy::indexing_slicing)]
- `crates/rs_cam_viz/src/app/mcp/simulation.rs:76` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/app/mcp/simulation.rs:853` #[allow(clippy::too_many_arguments)]
- `crates/rs_cam_viz/src/app/mcp/simulation.rs:1049` #[allow(clippy::indexing_slicing)] // pos came from pass_index_of
- `crates/rs_cam_viz/src/app/mcp/simulation.rs:1101` #[allow(clippy::indexing_slicing)] // SAFETY: `kind.index()` is bounded by COUNT.
- `crates/rs_cam_viz/src/compute/worker.rs:3` #[allow(
- `crates/rs_cam_viz/src/compute/worker.rs:12` #[allow(
- `crates/rs_cam_viz/src/compute/worker.rs:20` #[allow(
- `crates/rs_cam_viz/src/compute/worker.rs:324` #[allow(clippy::large_enum_variant)]
- `crates/rs_cam_viz/src/compute/worker.rs:922` #[allow(clippy::expect_used)]
- `crates/rs_cam_viz/src/compute/worker/gen_parity_p0_tests.rs:269` #[allow(dead_code)]
- `crates/rs_cam_viz/src/compute/worker/test_fixture.rs:235` #[allow(clippy::needless_pass_by_value)]

## TODO / FIXME / XXX / HACK in production code (3)

- `crates/rs_cam_core/src/feeds/vendor_normalize.rs:315` // minimal practical impact today. TODO Phase E+: extend
- `crates/rs_cam_core/src/feeds/vendor_normalize.rs:351` // TODO Phase 3+: plumb Rockwell HardnessKind through
- `crates/rs_cam_core/src/compute/execute/tests.rs:199` /// F-XXX regression, re-based by L2: adaptive3d's planner supports a
