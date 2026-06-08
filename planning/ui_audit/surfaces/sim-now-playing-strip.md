surface: Inspector › Now-playing strip + tool-load badges
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: inline-widget
job: Show the currently-playing toolpath's name plus its chipload/power/L-D (or drill-gate) load badges, with optimize/jump-to-start.
opens-from: Bottom of the project overview when playback is inside a boundary
controls:
  - read-tool-load-chipload-badge
  - read-tool-load-power-badge
  - read-tool-load-deflection-badge
  - read-drill-gates
  - open-optimize-modal
  - jump-to-toolpath-start
reads-state: sim.current_boundary(), load_report.per_toolpath[], sim.cached_chipload_envelopes(), session.machine().{power,safety_factor}, verdict.{chipload,power,deflection,drill_gates}
writes-state: emits OpenOptimizeModal, SimJumpToMove
confusable-with: op-list per-row status flags (same verdict, short-label form), signal-spine "Playing:" header, verdict HUD load pills (project-wide aggregate of the same verdicts)
recommendation-sources-touched: vendor-lut (chipload bound from LUT envelope/floor; power cap from machine; L/D from DEFLECTION_SAFE_LD_RATIO const), sim-feedback (peaks measured in sim)
health: yellow — tool-load verdict is surfaced 3 ways (full badges here, short flags in op-list, aggregate pills in HUD); chipload cap source switches between LUT floor and cap depending on burn-risk, legible only in the tooltip.
