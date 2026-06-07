surface: Inspector › Focused hotspot card
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: inline-widget
job: Show the metrics for the one hotspot the user clicked and offer jump/optimize/clear.
opens-from: Rendered at top of reactive inspector when sim.focused_hotspot_data() is Some (set by clicking a 3D pin, signal-graph dot, or a Top-hotspots row); pre-empts the project overview
controls:
  - read-hotspot-metrics
  - jump-to-move
  - open-optimize-modal
  - clear-focused-hotspot
reads-state: sim.focused_hotspot_data() (toolpath_id, move range, wasted_runtime_s, peak_chipload, peak_axial_doc, average_engagement, position)
writes-state: sim.debug.focused_hotspot (clear); emits SimJumpToMove, OpenOptimizeModal
confusable-with: sim-issue-card (near-identical framed card with Jump/Optimize buttons), Top-hotspots list rows, Selected-span "Findings in this span" hotspot rows
recommendation-sources-touched: sim-feedback (hotspot metrics come from the simulation cut trace); Optimize button is the entry to the suggestion/optimize flow
health: yellow — one of three near-identical hotspot/issue cards (P4 confusable) and one of many "Optimize" entry points scattered across the inspector.
