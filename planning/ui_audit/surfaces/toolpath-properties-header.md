surface: Toolpath Properties Header
file: crates/rs_cam_viz/src/ui/properties/mod.rs (draw_toolpath_panel, ~2501-2744)
kind: panel
job: Identity + IO + generate + diagnostics ribbon for the selected toolpath, shown above the tab bar regardless of active tab.
opens-from: Selecting a toolpath (Selection::Toolpath) populates the right-hand properties panel
controls:
  - rename-toolpath
  - select-tool
  - select-input-model
  - select-faces
  - clear-faces
  - set-stock-source
  - generate-toolpath
  - show-toolpath-diagnostics
reads-state: entry.name, entry.tool_id, entry.model_id, entry.face_selection, entry.stock_source, entry.status, entry.result, model_has_enriched, model_is_step_missing_brep, validation errors, collect_diagnostics(entry, tool, stale_defaults, height_ctx, load_verdict)
writes-state: entry.name, entry.tool_id, entry.model_id, entry.face_selection (+stale_since), entry.stock_source (+stale_since); emits GenerateToolpath
confusable-with: Operations Queue card (also has tool name, status, Generate); Params-tab Generate is the same op
recommendation-sources-touched: sim-feedback (diagnostics ribbon surfaces load_verdict + NeedsSimulation tiers)
health: yellow — mixes four concerns in one always-on header (identity, geometry/face IO, stock-source linking, generate+diagnostics); the diagnostics ribbon (actionable/stateful/hints) is the richest provenance surface but is buried under unrelated IO fields (P2 grouping, P3 dig-deeper).
