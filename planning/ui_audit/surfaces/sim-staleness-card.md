surface: Verification staleness card
file: crates/rs_cam_viz/src/ui/sim_op_list.rs
kind: inline-widget
job: Warn that sim results are stale after parameter edits and offer a one-click re-run.
opens-from: Rendered in the left panel when sim.is_stale(edit_counter) is true
controls:
  - run-simulation
reads-state: sim.is_stale(gui.edit_counter)
writes-state: emits RunSimulation
confusable-with: sim-setup-and-run (Run button), viewport-overlay (Re-run button), project-overview stale notice (a non-actionable text twin of this card)
recommendation-sources-touched: none
health: yellow — duplicates both the staleness signal (also shown as plain text in project-overview) and the Run button (3 other homes); same concern in 2 cards plus 1 text line.
