surface: Inspector › Project overview
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: panel
job: Default whole-cut summary — cycle time, move/op/distance totals, a pass/warn banner, findings counts, and per-issue-kind breakdown.
opens-from: Default body of the reactive inspector when results exist and no hotspot/issue is focused
controls:
  - read-cycle-time
  - read-move-count
  - read-operation-count
  - read-cut-distance
  - read-rapid-distance
  - read-air-cut-pct
  - read-collision-count
  - read-findings-counts
  - open-optimize-project
  - read-stale-flag
reads-state: sim aggregate stats, load_report.summary(), sim.checks.{rapid_collisions,holder_collision_count}, sim.results.cut_trace.summary (air_cut_time_s,total_runtime_s), sim.issues(), sim.is_stale()
writes-state: emits OpenOptimizeProject; refreshes sim.debug.span_aggregates cache
confusable-with: sim-verdict-hud (bottom panel — also a project-wide pass/warn/collision/issue rollup), preflight modal (also rolls up collisions + tool-load), staleness card (re-shows the same stale flag)
recommendation-sources-touched: vendor-lut (findings within/exceeds counts come from ToolLoadReport), sim-feedback (air-cut/collision counts)
health: yellow — large flat stack of grids (P3 dump) and it is the third project-wide rollup (verdict HUD + preflight say the same thing differently); banner rule is hand-duplicated from the MCP rule rather than shared.
