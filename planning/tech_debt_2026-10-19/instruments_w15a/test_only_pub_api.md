# `pub` items referenced only from tests/benches/examples (44)

Production word count is 1 (the definition) and the test-side count is > 0. Some are deliberate fixtures (`test_fixture`); the rest are test-only API leaking as public surface. Verify with rg; same blind spots as dead_pub_surface.md.

| kind | name | file:line | test refs |
|---|---|---|---|
| fn | `cell_cap_under` | `crates/rs_cam_core/src/budget/grid.rs:78` | 3 |
| fn | `check_estimate` | `crates/rs_cam_core/src/budget/guard.rs:246` | 3 |
| fn | `is_populated` | `crates/rs_cam_core/src/compute/sim_prefix.rs:844` | 8 |
| fn | `has_split` | `crates/rs_cam_core/src/compute/toolpath_stats.rs:895` | 2 |
| fn | `tip_float_measured` | `crates/rs_cam_core/src/compute/toolpath_stats.rs:1870` | 3 |
| fn | `retract_trip_measurement` | `crates/rs_cam_core/src/compute/toolpath_stats.rs:1884` | 4 |
| fn | `render_mesh_composite` | `crates/rs_cam_core/src/export/fingerprint.rs:858` | 10 |
| fn | `composite_panel_layout` | `crates/rs_cam_core/src/export/fingerprint.rs:1014` | 16 |
| fn | `refusal_reason` | `crates/rs_cam_core/src/feeds/extrapolation.rs:359` | 2 |
| fn | `unit_family` | `crates/rs_cam_core/src/feeds/feed_explanation.rs:198` | 2 |
| fn | `predicted_gate_observation_mm` | `crates/rs_cam_core/src/feeds/feed_explanation.rs:326` | 1 |
| fn | `sample_with_probe_diameter` | `crates/rs_cam_core/src/finish/classify_probe.rs:284` | 2 |
| fn | `projected_xy_area_mm2` | `crates/rs_cam_core/src/finish/finish_planner.rs:278` | 5 |
| fn | `is_shipped` | `crates/rs_cam_core/src/finish/scallop.rs:722` | 4 |
| fn | `uncut_core` | `crates/rs_cam_core/src/finish/scallop.rs:968` | 4 |
| fn | `override_needed` | `crates/rs_cam_core/src/gcode/mod.rs:132` | 1 |
| fn | `reach_map_for_mesh` | `crates/rs_cam_core/src/maps/reach_map.rs:1854` | 31 |
| fn | `reset_drop_call_count` | `crates/rs_cam_core/src/maps/tier_map.rs:183` | 10 |
| fn | `kind_id` | `crates/rs_cam_core/src/material/force_line.rs:331` | 2 |
| const | `PRIZE_CLOSE_BELOW` | `crates/rs_cam_core/src/metrology/census.rs:97` | 1 |
| const | `PRIZE_ABOVE_LITERATURE` | `crates/rs_cam_core/src/metrology/census.rs:105` | 1 |
| fn | `rest_op` | `crates/rs_cam_core/src/metrology/costing.rs:146` | 6 |
| fn | `mesh_area_mm2` | `crates/rs_cam_core/src/metrology/floor.rs:200` | 2 |
| fn | `tessellate_heightfield` | `crates/rs_cam_core/src/metrology/monge.rs:548` | 4 |
| fn | `with_holes_closed` | `crates/rs_cam_core/src/polygon.rs:168` | 11 |
| fn | `planned_tier_boundary_polys` | `crates/rs_cam_core/src/session/multitool.rs:725` | 6 |
| fn | `dexel_stock_to_top_surface_mesh` | `crates/rs_cam_core/src/stock/dexel_mesh.rs:166` | 7 |
| fn | `resolve_flats` | `crates/rs_cam_core/src/surface/flow_accum.rs:213` | 9 |
| fn | `d8_accumulation` | `crates/rs_cam_core/src/surface/flow_accum.rs:397` | 15 |
| fn | `is_covered` | `crates/rs_cam_core/src/surface/slope.rs:80` | 10 |
| fn | `filtered_out` | `crates/rs_cam_core/src/tool_load/verdict.rs:1305` | 1 |
| fn | `export_single_toolpath_from_session` | `crates/rs_cam_viz/src/io/export.rs:821` | 5 |
| fn | `all_defaults` | `crates/rs_cam_viz/src/mcp_bridge.rs:787` | 2 |
| fn | `generate_all_without_peer` | `crates/rs_cam_viz/src/mcp_server.rs:1547` | 3 |
| fn | `bound_place` | `crates/rs_cam_viz/src/ui/components/histogram.rs:528` | 4 |
| fn | `is_encoding` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:144` | 1 |
| fn | `key_colours` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:958` | 2 |
| fn | `swatch_colours` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:963` | 3 |
| fn | `request_compute` | `crates/rs_cam_viz/src/ui/overlays/panel.rs:484` | 5 |
| fn | `none_reason` | `crates/rs_cam_viz/src/ui/properties/operations/registry.rs:104` | 1 |
| fn | `draw_trace_badge` | `crates/rs_cam_viz/src/ui/sim_debug.rs:23` | 2 |
| const | `INK_00` | `crates/rs_cam_viz/src/ui/tokens.rs:90` | 1 |
| const | `LANE_SCALE` | `crates/rs_cam_viz/src/ui/tokens.rs:332` | 2 |
| const | `DOCK_STACK_MAX_HEIGHT_FRACTION` | `crates/rs_cam_viz/src/ui/viewport_overlay.rs:80` | 1 |
