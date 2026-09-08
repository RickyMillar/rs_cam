surface: Overlays panel
file: crates/rs_cam_viz/src/ui/overlays/panel.rs (list: crates/rs_cam_viz/src/ui/overlays/registry.rs)
kind: panel
job: List EVERY viewport overlay in four groups, switch each one, and — for an overlay that cannot draw — say in one line why and offer the button that makes it drawable.
opens-from: The `Overlays (n)` button on the viewport strip, or the `O` shortcut. `Shift+O` pins it as a column inside the viewport; unpinned it floats over the 3D view.
default-colour-source: model = Reach, ON in Toolpaths (operator ruling 2026-09-08: "show the reach map when ANY finishing op is selected"); the rest heatmap is OFF in every workspace and switching it on clears Reach. Simulated stock = Solid. Move lines = Palette.
controls:
  - toggle-overlay (38 rows: Geometry 12, Toolpath 9, Regions 5, Analysis 12)
  - set-surface-colour-source (model / simulated stock / move lines, one at a time)
  - stock-opacity
  - run-overlay-compute-action (Run simulation, Run collision check, Plan…, Generate all, Record & re-generate, Rest Analysis…)
  - pin-overlays-panel
  - close-overlays-panel
reads-state: the whole AppState, every frame and cached nowhere — the multi-tool planner writes show_tier_preview on its own and a workspace switch rewrites a dozen flags behind the panel's back
writes-state: viewport.{show_grid,show_model,show_stock,show_stock_solid,show_origin_axes,show_datum,show_fixtures,show_keep_outs,show_alignment_pins,show_flip_axis,show_polygons,show_orientation_gizmo,show_cutting,show_rapids,show_entry_markers,show_height_planes,show_tool_profile_preview,span_kind_filter.*,show_rest_heatmap,show_tier_preview,show_reach_map,show_sim_stock,show_collisions,show_tool_deflection,toolpath_color_mode}, simulation.{stock_viz_mode,stock_opacity,debug.enabled,debug.highlight_active_item}, overlays.{open,pinned,groups}, gui.pending_toolpath_tab (the Rest Analysis navigation); emits RunSimulation, RunCollisionCheck, OpenMultitoolPlanner, GenerateAll, SetGeneratorTraceCaptureAll
confusable-with: the per-toolpath eye / C / R / bullseye on each operation row — per-OBJECT state that deliberately stays there; the panel carries a pointer to it
recommendation-sources-touched: vendor-lut (the Advance-per-tooth legend classes come from VendorChiploadBand::classify, the same classifier the move colours use)
health: green — the model surface holds exactly one colour source at a time, and a sentry pins WHICH side owns the default (`reach_owns_the_model_surface_default_and_the_rest_heatmap_is_off`), because reversing that pair silently would take a shipped answer off the screen. One declarative registry feeds the panel, the MCP `set_ui_view` `overlays` map and the completeness sentries (`crates/rs_cam_viz/tests/overlays_registry.rs`), so the three cannot disagree about what exists, what it is called or why it cannot draw. Each row records its mechanism (draw-time or upload-time) and an upload-time row must be carried in the composite `overlay_upload_key` detector — a sentry asserts it, because an upload-time flag with no trigger is a dead control that looks live. Three rows are listed permanently disabled with the reason "not drawn yet (no renderer)" / "drawn by Regions ▸ Tier map": derived rest regions, the boundary outline and the planner islands. Stock opacity is a slider under the Simulated stock row rather than a registry row.
