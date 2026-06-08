surface: Viewport overlay toolbar
file: crates/rs_cam_viz/src/ui/viewport_overlay.rs
kind: menu
job: Floating 3D-viewport toolbar for camera presets, render/projection mode, visibility toggles, isolation, compute activity, and per-workspace actions.
opens-from: Overlaid on the top of the 3D viewport in every workspace
controls:
  - set-view-preset
  - reset-view
  - set-render-mode
  - toggle-projection
  - show-grid
  - show-stock
  - show-fixtures
  - show-polygons
  - show-cutting-moves
  - show-rapid-moves
  - show-collisions
  - filter-spankind
  - show-tool-profile-ghost
  - toolpath-color-mode
  - toggle-isolate-toolpath
  - clear-isolation
  - cancel-compute
  - generate-all
  - run-simulation
  - reset-simulation
reads-state: viewport.{render_mode,show_grid,show_stock,show_fixtures,show_polygons,show_cutting,show_rapids,show_collisions,show_tool_profile_preview,span_kind_filter,toolpath_color_mode}, projection, isolated_name, lanes[]
writes-state: viewport.{render_mode,show_grid,show_stock,show_fixtures,show_polygons,show_cutting,show_rapids,show_collisions,show_tool_profile_preview,span_kind_filter,toolpath_color_mode}; emits SetViewPreset, ResetView, ToggleProjection, ToggleIsolateToolpath, ClearIsolation, CancelCompute, GenerateAll, RunSimulation, ResetSimulation
confusable-with: Inspector View section (show_stock/show_cutting/show_rapids written in BOTH), sim-setup-and-run + staleness card (RunSimulation also there)
recommendation-sources-touched: vendor-lut (Chipload color mode buckets per-segment chipload vs matched vendor row window)
health: red — show_stock, show_cutting, show_rapids are authoritatively writable here AND in the Inspector View section (P1 duplicate write paths to identical viewport fields); RunSimulation is one of four homes for the run action; the toolbar mixes camera, visibility, isolation, compute-cancel, and workspace CRUD actions in one row (P2).
