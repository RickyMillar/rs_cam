surface: Verdict HUD (info pills)
file: crates/rs_cam_viz/src/ui/sim_timeline.rs
kind: inline-widget
job: Glanceable project-wide rollup of load ok/unmodeled/exceeds/approx + collisions + issues + traces as colored pills.
opens-from: Framed pill row between the transport and the boundary timeline in the bottom panel
controls:
  - read-load-within-count
  - read-load-unmodeled-count
  - read-load-exceeds-count
  - read-load-approx-count
  - read-collision-count
  - read-issue-count
  - read-trace-count
reads-state: load_report.per_toolpath[] (per-criterion state+confidence), sim.checks.{rapid_collisions,holder_collision_count}, sim.issues(), gui.toolpath_rt[].{debug_trace,semantic_trace}
writes-state: none (events arg unused)
confusable-with: project-overview Findings grid (same counts, different denominator — HUD counts per-criterion, overview counts per-toolpath), preflight modal rollup
recommendation-sources-touched: vendor-lut (load verdicts), sim-feedback (collisions/issues)
health: red — its load counts are per-criterion (3× per TP) while project-overview Findings are per-toolpath, so the SAME concept shows DIFFERENT numbers in two panels (P1+P7 confusable); read-only despite taking a mut events sink.
