surface: Inspector › Generation Metrics
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: panel
job: Report how the toolpath was built — generator phase timings, dominant span, hotspot count, semantic item count.
opens-from: CollapsingHeader "Generation Metrics" in the Inspector right panel (default closed)
controls:
  - read-generator-trace-summary
  - read-linked-debug-span
  - read-semantic-item-count
reads-state: gui.toolpath_rt[].{debug_trace,semantic_trace}, trace.summary.{total_elapsed_us,dominant_span_label,dominant_span_elapsed_us}, trace.hotspots, sim.active_debug_span(), sim.current_debug_annotation()
writes-state: none
confusable-with: sim-selection-details, Top-hotspots collapsing list (also surfaces hotspot counts), sim-verdict-hud (also shows a traces count)
recommendation-sources-touched: none
health: green — clearly scoped read-only build-time diagnostics, explicitly orthogonal to the structural span tree per its own comment.
