# `pub` items referenced only from tests/benches/examples (36)

Production word count is 1 (the definition) and the test-side count is > 0. Some are deliberate fixtures (`test_fixture`); the rest are test-only API leaking as public surface. Verify with rg; same blind spots as dead_pub_surface.md.

| kind | name | file:line | test refs |
|---|---|---|---|
| fn | `adaptive_3d_toolpath_annotated` | `crates/rs_cam_core/src/adaptive3d/mod.rs:455` | 5 |
| fn | `has_split` | `crates/rs_cam_core/src/compute/config.rs:840` | 2 |
| fn | `tip_float_measured` | `crates/rs_cam_core/src/compute/config.rs:1477` | 3 |
| fn | `retract_trip_measurement` | `crates/rs_cam_core/src/compute/config.rs:1491` | 4 |
| fn | `is_populated` | `crates/rs_cam_core/src/compute/sim_prefix.rs:606` | 7 |
| fn | `render_mesh_composite` | `crates/rs_cam_core/src/export/fingerprint.rs:831` | 2 |
| fn | `composite_panel_layout` | `crates/rs_cam_core/src/export/fingerprint.rs:980` | 7 |
| fn | `unit_family` | `crates/rs_cam_core/src/feeds/feed_explanation.rs:268` | 2 |
| fn | `predicted_gate_observation_mm` | `crates/rs_cam_core/src/feeds/feed_explanation.rs:394` | 1 |
| fn | `write_to` | `crates/rs_cam_core/src/feeds/suggest.rs:1166` | 2 |
| fn | `preview_field_apply` | `crates/rs_cam_core/src/feeds/suggest.rs:1282` | 3 |
| fn | `sample_with_probe_diameter` | `crates/rs_cam_core/src/finish/classify_probe.rs:287` | 2 |
| fn | `projected_xy_area_mm2` | `crates/rs_cam_core/src/finish/finish_planner.rs:256` | 5 |
| fn | `reset_surface_build_count` | `crates/rs_cam_core/src/finish/finish_setup.rs:367` | 4 |
| fn | `is_shipped` | `crates/rs_cam_core/src/finish/scallop.rs:706` | 4 |
| fn | `uncut_core` | `crates/rs_cam_core/src/finish/scallop.rs:1813` | 4 |
| type | `V2` | `crates/rs_cam_core/src/geo.rs:14` | 13 |
| fn | `reach_map_for_mesh` | `crates/rs_cam_core/src/maps/reach_map.rs:1829` | 25 |
| fn | `reset_drop_call_count` | `crates/rs_cam_core/src/maps/tier_map.rs:170` | 6 |
| fn | `label_at` | `crates/rs_cam_core/src/maps/tier_map.rs:496` | 11 |
| const | `PRIZE_CLOSE_BELOW` | `crates/rs_cam_core/src/metrology/census.rs:93` | 1 |
| const | `PRIZE_ABOVE_LITERATURE` | `crates/rs_cam_core/src/metrology/census.rs:101` | 1 |
| fn | `mesh_area_mm2` | `crates/rs_cam_core/src/metrology/floor.rs:200` | 2 |
| fn | `tessellate_heightfield` | `crates/rs_cam_core/src/metrology/monge.rs:548` | 4 |
| fn | `with_holes_closed` | `crates/rs_cam_core/src/polygon.rs:166` | 6 |
| fn | `planned_tier_boundary_polys` | `crates/rs_cam_core/src/session/multitool.rs:628` | 3 |
| fn | `dexel_stock_to_top_surface_mesh` | `crates/rs_cam_core/src/stock/dexel_mesh.rs:111` | 4 |
| fn | `priority_flood_epsilon` | `crates/rs_cam_core/src/surface/flow_accum.rs:135` | 14 |
| fn | `resolve_flats` | `crates/rs_cam_core/src/surface/flow_accum.rs:203` | 5 |
| fn | `d8_accumulation` | `crates/rs_cam_core/src/surface/flow_accum.rs:385` | 12 |
| fn | `is_covered` | `crates/rs_cam_core/src/surface/slope.rs:78` | 3 |
| fn | `filtered_out` | `crates/rs_cam_core/src/tool_load/verdict.rs:697` | 1 |
| fn | `generate_all_without_peer` | `crates/rs_cam_viz/src/mcp_server.rs:1300` | 3 |
| fn | `draw_trace_badge` | `crates/rs_cam_viz/src/ui/sim_debug.rs:23` | 2 |
| const | `INK_00` | `crates/rs_cam_viz/src/ui/tokens.rs:92` | 1 |
| const | `LANE_SCALE` | `crates/rs_cam_viz/src/ui/tokens.rs:317` | 2 |
