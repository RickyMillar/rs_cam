surface: Setup & run (Verification left panel header block)
file: crates/rs_cam_viz/src/ui/sim_op_list.rs
kind: panel
job: Choose what the next simulation run records (cut metrics, generator trace, dexel resolution) and launch the run.
opens-from: Top of the Simulation-workspace left panel (always visible when any toolpath is enabled)
controls:
  - capture-cut-metrics
  - capture-generator-trace
  - sim-resolution
  - sim-resolution-auto
  - run-simulation
reads-state: sim.metric_options.enabled, sim.metric_options.capture_arc_engagement, sim.resolution, sim.auto_resolution, session.toolpath_configs[].enabled, session.toolpath_configs[].debug_options.enabled, session.stock_config.{x,y}
writes-state: sim.metric_options.enabled, sim.metric_options.capture_arc_engagement, sim.resolution, sim.auto_resolution; emits SetGeneratorTraceCaptureAll, RunSimulation
confusable-with: viewport-overlay (also has Re-run button), sim-staleness-card (also has Re-run Simulation button), sim-inspector-view (the comment explicitly splits "recording" toggles here from "display" toggles there)
recommendation-sources-touched: none
health: yellow — single concern (recording config + run) but the Run button is duplicated in 3 other surfaces (overlay Re-run, staleness card, project-overview not), and "Capture cutting metrics" silently force-sets capture_arc_engagement, an invisible coupling.
