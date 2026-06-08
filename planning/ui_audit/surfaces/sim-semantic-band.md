surface: Semantic timeline band
file: crates/rs_cam_viz/src/ui/sim_timeline.rs
kind: inline-widget
job: Per-toolpath strip of semantic-trace segments + generator annotations + air-cut/low-engagement issue dots; click to pin item / jump / route to an analytics tab.
opens-from: 10px painted strip below the span ribbon, only when sim.debug.enabled and a current boundary exists
controls:
  - read-semantic-segments
  - read-annotations
  - read-cut-issue-dots
  - pin-semantic-item
  - jump-to-move
reads-state: gui.toolpath_rt[].{semantic_trace,debug_trace}, sim.debug.semantic_indexes, sim.results.cut_trace.issues, sim.playback.current_move, active_semantic
writes-state: sim.debug.{focused_issue_index,focused_hotspot} (clear), sim.analytics_tab (DebugTrace/CutQuality), pin/clear semantic item; emits SimJumpToMove
confusable-with: boundary-timeline + span-ribbon (third stacked strip), op-list semantic outline (same semantic trace, tree form), issue-card / cut-issue dots elsewhere
recommendation-sources-touched: sim-feedback (cut-issue dots from sim trace)
health: yellow — local-move coordinate space differs from the two strips stacked above it (won't align), and clicking it silently switches analytics_tab to tabs not present in the audited files; gated behind the debug toggle so most users never see it.
