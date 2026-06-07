surface: Inspector › Top hotspots list
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: panel
job: Project-wide triage list of the most time-wasting hotspots, sorted by wasted runtime, click to focus+jump.
opens-from: CollapsingHeader "Top hotspots (N)" inside the project overview (default closed)
controls:
  - read-hotspot-list
  - focus-hotspot
  - jump-to-move
reads-state: sim.results.cut_trace.hotspots[] (toolpath_id, move_start, wasted_runtime_s, peak_chipload)
writes-state: sim.debug.focused_hotspot; emits SimJumpToMove
confusable-with: Selected-span "Findings in this span" hotspot list (span-scoped twin), focused-hotspot card (single-hotspot detail), signal-spine gate-trip dots
recommendation-sources-touched: sim-feedback (hotspots from cut trace)
health: yellow — duplicate hotspot list with the Selected-span findings list (project-wide vs span-scoped), both rendering near-identical rows in the same panel.
