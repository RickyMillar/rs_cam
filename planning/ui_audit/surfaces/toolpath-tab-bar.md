surface: Toolpath Tab Bar
file: crates/rs_cam_viz/src/ui/properties/mod.rs (ToolpathTab, draw_toolpath_tabs ~2319, compute_tab_badges ~2066)
kind: tab
job: Switch the properties panel between Params / Feeds / Heights / Dressups, with per-tab warning badges.
opens-from: Always shown in the toolpath properties panel below the header
controls:
  - switch-toolpath-tab
reads-state: per-tab badge state (entry.feeds_result.warnings/power_limited, resolved heights ordering, actionable diagnostics), memory(tp_tab)
writes-state: memory(tp_tab) active tab
confusable-with: none
recommendation-sources-touched: sim-feedback (Dressups badge derives from Current diagnostics incl. tool-load gates)
health: green — clean concern split into 4 tabs (P2), badges give summary->detail signal of where attention is needed (P3); only nit is label "Mods"->"Dressups" mismatch in code.
