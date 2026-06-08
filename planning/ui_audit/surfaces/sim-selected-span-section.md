surface: Inspector › Selected span section
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: panel
job: Show sample aggregates (engagement, chipload, axial DOC, MRR) plus in-scope hotspots/issues for the span the ribbon is locked to or the playhead is inside.
opens-from: Bottom of the project overview when a cut trace exists; driven by sim.debug.span_scope or the playhead span
controls:
  - read-span-engagement
  - read-span-chipload
  - read-span-axial-doc
  - read-span-mrr
  - read-span-sample-count
  - clear-span-lock (Follow playhead)
  - focus-hotspot
  - jump-to-move
reads-state: sim.debug.span_scope.{span_id,toolpath_id}, sim.focused_toolpath()/current_boundary(), gui.toolpath_rt[].result.spans(), sim.debug.span_aggregates, trace.hotspots, issues[]
writes-state: sim.debug.span_scope (clear via Follow playhead), sim.debug.focused_hotspot; emits SimJumpToMove
confusable-with: sim-selection-details (semantic-item detail twin), Top-hotspots list (project-wide hotspot twin), span-ribbon (the thing that drives this section's lock)
recommendation-sources-touched: sim-feedback (all metrics from cut-trace samples)
health: yellow — its lock state is driven by a different surface (span-ribbon) with no visible affordance here besides a "locked" tag; chipload/engagement/DOC are also shown as continuous tracks in the signal spine (P1: same metrics, two homes).
