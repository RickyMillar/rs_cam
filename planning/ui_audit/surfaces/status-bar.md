surface: status-bar
file: crates/rs_cam_viz/src/ui/status_bar.rs
kind: inline-widget
job: Show read-only at-a-glance project state at the bottom: model/triangle counts, toolpath done/total, compute-lane chips, sim-available, collision count, and dirty flag.
opens-from: always visible, drawn at the bottom of the window
controls:
  - display-model-counts
  - display-toolpath-progress
  - display-compute-lanes
  - display-sim-available
  - display-collision-count
  - display-dirty-flag
reads-state: state.session.models(), state.gui.toolpath_rt, state.session.toolpath_configs(), lanes (LaneSnapshot[3]), state.simulation.has_results(), collision_count arg, state.gui.dirty
writes-state: none (records lane chips into the automation snapshot via automation::record but emits no AppEvent)
confusable-with: workspace-bar Simulation/Setup badges (collision count + sim-available shown in both); none of the status-bar items are clickable so confusion is mild
recommendation-sources-touched: sim-feedback (collision count + SIM marker come from simulation results)
health: yellow — purely informational and clean, but collision-count and sim-available duplicate the workspace-bar badges (P1), and the three lane chips pack queue depth / current job / elapsed into one prose string (P3).
