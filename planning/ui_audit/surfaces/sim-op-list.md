surface: Operation list (Verification left panel rows)
file: crates/rs_cam_viz/src/ui/sim_op_list.rs
kind: panel
job: List every toolpath grouped by setup with per-op include/visibility/jump controls and per-op simulation status flags.
opens-from: Body of the Simulation-workspace left panel once results exist
controls:
  - toggle-toolpath-in-sim-selection
  - jump-to-toolpath-start
  - toolpath-visibility (eye/cut/rapid/isolate via toolpath_row_controls)
  - toggle-span-outline
  - jump-to-span-start
  - jump-to-span-end
  - jump-to-semantic-item-start
  - jump-to-semantic-item-end
  - read-toolpath-status-flags
reads-state: sim.boundaries(), sim.setup_boundaries(), sim.selected_toolpaths(), sim.focused_toolpath(), sim.issues(), load_report.per_toolpath[], gui.toolpath_rt[].{result,semantic_trace,visible}, sim.debug.{enabled,toolpath_expanded,semantic_indexes,span_scope}
writes-state: sim.debug.toolpath_expanded, sim.debug.pinned_semantic_item (pin/clear), sim.debug.span_scope (cleared on span click); emits RunSimulation, RunSimulationWith, SimJumpToOpStart, SimJumpToMove, viewport visibility events
confusable-with: project-overview "Now playing" strip (also shows per-TP tool-load verdict + jump), signal-spine "Playing:" header, span-ribbon (also a span navigator that jumps + scopes)
recommendation-sources-touched: vendor-lut (status flags derive from ToolpathLoadVerdict chipload/power/deflection criteria, which are LUT-backed)
health: yellow — mixes 4 concerns in each row (sim-inclusion checkbox, viewport visibility, playback jump, and a nested span/semantic tree), and the structural-vs-semantic outline is a confusable dual-mode tree; whole-card click as a jump target competes with many inner click targets.
