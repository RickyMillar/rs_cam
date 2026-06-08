surface: Boundary timeline
file: crates/rs_cam_viz/src/ui/sim_timeline.rs
kind: inline-widget
job: Project-wide painted bar of per-op segments with progress fill, collision/tool-load safety markers, playhead, and click/drag-to-seek (snapping to safety markers).
opens-from: Custom-painted bar below the verdict HUD in the bottom panel
controls:
  - seek-playback
  - jump-to-safety-marker
  - read-collision-markers
  - read-tool-load-markers
reads-state: sim.boundaries(), sim.playback.current_move, sim.checks.{collision_report,rapid_collision_move_indices}, load_report.per_toolpath[], sim.focused_toolpath()
writes-state: sim.playback.{current_move,playing,scrub_drag_active}, sim.analytics_tab (set to Safety on marker click); emits SimJumpToMove
confusable-with: span-ribbon (sits directly under it, also clickable, also has a playhead, but jumps+scopes spans instead of seeking), semantic-band (third stacked timeline strip), signal-spine X axis (different coordinate space — markers won't align)
recommendation-sources-touched: vendor-lut (tool-load markers), sim-feedback (collision markers)
health: yellow — three stacked timeline strips (boundary / span-ribbon / semantic-band) with overlapping playheads and different click semantics (seek vs scope vs pin) is a P4 confusable; sets analytics_tab to a tab not in the audited files.
