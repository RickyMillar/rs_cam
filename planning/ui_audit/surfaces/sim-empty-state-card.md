surface: Verification empty-state card
file: crates/rs_cam_viz/src/ui/sim_op_list.rs
kind: inline-widget
job: Tell the user there are no simulation results yet and route them to either run a sim or go generate toolpaths.
opens-from: Rendered in the left panel when sim.boundaries() is empty
controls:
  - go-to-toolpaths
reads-state: sim.boundaries(), session.toolpath_configs[].enabled, gui.toolpath_rt[].result
writes-state: emits SwitchWorkspace(Toolpaths)
confusable-with: sim-staleness-card (both are framed cards with a call-to-action button)
recommendation-sources-touched: none
health: green — clean empty-state with a single contextual action.
