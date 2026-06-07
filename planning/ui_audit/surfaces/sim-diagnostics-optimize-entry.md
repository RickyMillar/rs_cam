surface: sim-diagnostics-optimize-entry
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: inline-widget
job: Surface "Optimize" entry points in context from simulation findings (hotspot card, issue card, per-toolpath boundary row, and a project-level "Optimize all exceeding toolpaths").
opens-from: rendered inside the simulation diagnostics panel after a sim runs (hotspot card :411, focused-issue card :457, findings project button :665, per-toolpath boundary :866).
controls:
- open-optimize-modal (per-toolpath "Optimize" → OpenOptimizeModal)
- open-optimize-project ("⚡ Optimize all N exceeding toolpaths" → OpenOptimizeProject)
reads-state: simulation results, hotspots, issues, per-toolpath tool_load verdicts/badges, exceeding count
writes-state: none (pure launcher; opens optimize-modal / optimize-project)
confusable-with: the Feeds-tab "Open Feeds & Speeds modal" launcher (other recommend-this-toolpath entry); menu_bar "Optimize project…" (same OpenOptimizeProject from a different place)
recommendation-sources-touched: sim-feedback
health: green — this is the correct provenance-legible home: sim-feedback recommendations are launched from where the sim findings live (discovered-in-context). Main note: OpenOptimizeProject has two launch sites (here + menu_bar).
