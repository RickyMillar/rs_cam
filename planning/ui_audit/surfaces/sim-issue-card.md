surface: Inspector › Focused issue card
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: inline-widget
job: Show the air-cut / low-engagement issue at the current move with prev/next/jump/optimize navigation.
opens-from: Rendered when sim.current_issue() is Some and no hotspot is focused; pre-empts the project overview
controls:
  - read-issue
  - focus-prev-issue
  - focus-next-issue
  - jump-to-move
  - open-optimize-modal
reads-state: sim.current_issue(), issue.{kind,label,move_index,toolpath_id}
writes-state: emits SimJumpToMove, OpenOptimizeModal
confusable-with: sim-hotspot-card (same card shape + Jump/Optimize), Selected-span "Findings in this span" issue rows, semantic-band issue dots
recommendation-sources-touched: sim-feedback (issues derive from simulation); Optimize button enters the optimize/suggestion flow
health: yellow — duplicate card pattern with the hotspot card; Optimize entry point appears here, on the hotspot card, on the now-playing strip, and in Findings (P1/P4).
