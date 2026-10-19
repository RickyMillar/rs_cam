# `pub` items referenced only from tests/benches/examples (44)

Production word count is 1 (the definition) and the test-side count is > 0. Some are deliberate fixtures (`test_fixture`); the rest are test-only API leaking as public surface. Verify with rg; same blind spots as dead_pub_surface.md.

| kind | name | file:line | test refs |
|---|---|---|---|
| fn | `adaptive_3d_toolpath_annotated` | `crates/rs_cam_core/src/adaptive3d/mod.rs:536` | 7 |
| fn | `cell_cap_under` | `crates/rs_cam_core/src/budget/grid.rs:78` | 3 |
| fn | `check_estimate` | `crates/rs_cam_core/src/budget/guard.rs:252` | 3 |
| fn | `is_populated` | `crates/rs_cam_core/src/compute/sim_prefix.rs:792` | 8 |
| fn | `has_split` | `crates/rs_cam_core/src/compute/toolpath_stats.rs:902` | 2 |
| fn | `tip_float_measured` | `crates/rs_cam_core/src/compute/toolpath_stats.rs:1876` | 3 |
| fn | `retract_trip_measurement` | `crates/rs_cam_core/src/compute/toolpath_stats.rs:1890` | 4 |
| fn | `render_mesh_composite` | `crates/rs_cam_core/src/export/fingerprint.rs:856` | 7 |
| fn | `composite_panel_layout` | `crates/rs_cam_core/src/export/fingerprint.rs:1009` | 13 |
| fn | `refusal_reason` | `crates/rs_cam_core/src/feeds/extrapolation.rs:359` | 2 |
| fn | `unit_family` | `crates/rs_cam_core/src/feeds/feed_explanation.rs:198` | 2 |
| fn | `predicted_gate_observation_mm` | `crates/rs_cam_core/src/feeds/feed_explanation.rs:326` | 1 |
| fn | `sample_with_probe_diameter` | `crates/rs_cam_core/src/finish/classify_probe.rs:288` | 2 |
| fn | `projected_xy_area_mm2` | `crates/rs_cam_core/src/finish/finish_planner.rs:278` | 5 |
| fn | `is_shipped` | `crates/rs_cam_core/src/finish/scallop.rs:724` | 4 |
| fn | `uncut_core` | `crates/rs_cam_core/src/finish/scallop.rs:970` | 4 |
| fn | `reach_map_for_mesh` | `crates/rs_cam_core/src/maps/reach_map.rs:1854` | 31 |
| fn | `reset_drop_call_count` | `crates/rs_cam_core/src/maps/tier_map.rs:176` | 10 |
| fn | `kind_id` | `crates/rs_cam_core/src/material/force_line.rs:331` | 2 |
| const | `PRIZE_CLOSE_BELOW` | `crates/rs_cam_core/src/metrology/census.rs:97` | 1 |
| const | `PRIZE_ABOVE_LITERATURE` | `crates/rs_cam_core/src/metrology/census.rs:105` | 1 |
| fn | `rest_op` | `crates/rs_cam_core/src/metrology/costing.rs:146` | 6 |
| fn | `mesh_area_mm2` | `crates/rs_cam_core/src/metrology/floor.rs:200` | 2 |
| fn | `tessellate_heightfield` | `crates/rs_cam_core/src/metrology/monge.rs:548` | 4 |
| fn | `with_holes_closed` | `crates/rs_cam_core/src/polygon.rs:166` | 7 |
| fn | `planned_tier_boundary_polys` | `crates/rs_cam_core/src/session/multitool.rs:721` | 3 |
| fn | `dexel_stock_to_top_surface_mesh` | `crates/rs_cam_core/src/stock/dexel_mesh.rs:165` | 4 |
| fn | `resolve_flats` | `crates/rs_cam_core/src/surface/flow_accum.rs:203` | 5 |
| fn | `d8_accumulation` | `crates/rs_cam_core/src/surface/flow_accum.rs:385` | 12 |
| fn | `is_covered` | `crates/rs_cam_core/src/surface/slope.rs:80` | 10 |
| fn | `chipload_envelope_for_toolpath` | `crates/rs_cam_core/src/tool_load/mod.rs:267` | 14 |
| fn | `filtered_out` | `crates/rs_cam_core/src/tool_load/verdict.rs:1212` | 1 |
| fn | `all_defaults` | `crates/rs_cam_viz/src/mcp_bridge.rs:782` | 2 |
| fn | `generate_all_without_peer` | `crates/rs_cam_viz/src/mcp_server.rs:1491` | 3 |
| fn | `bound_place` | `crates/rs_cam_viz/src/ui/components/histogram.rs:528` | 4 |
| fn | `is_encoding` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:144` | 1 |
| fn | `key_colours` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:945` | 2 |
| fn | `swatch_colours` | `crates/rs_cam_viz/src/ui/overlays/legend_rail.rs:950` | 3 |
| fn | `request_compute` | `crates/rs_cam_viz/src/ui/overlays/panel.rs:484` | 5 |
| fn | `none_reason` | `crates/rs_cam_viz/src/ui/properties/operations/registry.rs:94` | 1 |
| fn | `draw_trace_badge` | `crates/rs_cam_viz/src/ui/sim_debug.rs:23` | 2 |
| const | `INK_00` | `crates/rs_cam_viz/src/ui/tokens.rs:92` | 1 |
| const | `LANE_SCALE` | `crates/rs_cam_viz/src/ui/tokens.rs:328` | 2 |
| const | `DOCK_STACK_MAX_HEIGHT_FRACTION` | `crates/rs_cam_viz/src/ui/viewport_overlay.rs:80` | 1 |
