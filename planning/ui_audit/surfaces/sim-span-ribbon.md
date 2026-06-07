surface: Span ribbon
file: crates/rs_cam_viz/src/ui/sim_timeline.rs
kind: inline-widget
job: Subdivide the scope toolpath's segment into DepthPass (and Region sub-) blocks; click to scope the Selected section and scrub to the span start.
opens-from: 18px painted ribbon directly under the boundary timeline
controls:
  - set-span-scope
  - jump-to-span-start
  - read-span-subdivision
reads-state: sim.debug.span_scope.{toolpath_id,span_id}, sim.current_boundary(), gui.toolpath_rt[].result.spans(), sim.playback.current_move
writes-state: sim.debug.span_scope.{toolpath_id,span_id} (toggle); emits SimJumpToMove
confusable-with: boundary-timeline (immediately above, same width, also click-to-jump + playhead), op-list span outline tree (alternate span navigator), semantic-band
recommendation-sources-touched: none
health: yellow — it is the hidden driver of the Inspector Selected-span section with no cross-surface affordance linking the two; one of three+ span-navigation surfaces (ribbon, op-list tree, Selected header) for one concern (P2).
