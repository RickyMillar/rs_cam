surface: Export Readiness modal
file: crates/rs_cam_viz/src/ui/preflight.rs
kind: modal
job: Pre-export checklist that rolls up operations/sim/collisions/holder/tool-load/cycle-time and gates the Export G-code action behind overrides.
opens-from: Opened before G-code export (export gate)
controls:
  - read-operations-check
  - read-simulation-check
  - read-rapid-collision-check
  - read-holder-clearance-check
  - read-tool-load-check
  - read-cycle-time
  - go-to-toolpaths
  - go-to-simulation
  - run-collision-check
  - accept-unmodeled-load
  - accept-exceeded-load
  - confirm-export-risk
  - export-gcode
  - cancel-export
reads-state: session.toolpath_configs[], gui.toolpath_rt[].result, sim.{has_results,is_stale,checks}, gui.tool_load_overrides, project_load_report(session, sim_trace)
writes-state: gui.tool_load_overrides (via SetToolLoadOverride), egui temp "preflight_export_confirm"; emits SwitchWorkspace, RunCollisionCheck, SetToolLoadOverride, ExportGcodeConfirmed
confusable-with: project-overview + verdict HUD (both roll up collisions + tool-load with different counting), staleness card (re-states sim staleness)
recommendation-sources-touched: vendor-lut (tool-load model check + override panel reads ToolpathLoadVerdict), sim-feedback (collision/holder checks)
health: yellow — fourth surface to roll up the tool-load verdict (each counts differently); two-tier confirm (per-class override checkboxes AND a separate "I understand the risks" + "Export Anyway") is correct but dense; recomputes its own load_report rather than reusing the cached one other surfaces share (P6 duplicate compute).
