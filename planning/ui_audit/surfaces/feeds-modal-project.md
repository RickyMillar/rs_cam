surface: feeds-modal-project
file: crates/rs_cam_viz/src/ui/feeds_modal.rs
kind: modal
superseded-note: 2026-08-12 (TD3 wave A-4, Checkpoint I-1/I-3). The three batch
  applies below now route through feeds::suggest::apply with ApplyScope::Both, carry
  "changes the cut" on their faces, SKIP rows whose tool cannot run their operation and
  name them in a notification (pre-fix the sweep wrote to a refused row silently), and
  the table marks a refused row and shows "refused" in place of its Apply. See
  planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md and the A-4 log entry. This file
  is left otherwise as the 2026-06 audit snapshot recorded it.
job: Roll up every enabled toolpath's current-vs-recommended feeds into one table so the user can spot the biggest speedup opportunity and batch-apply LUT recommendations.
opens-from: "All toolpaths" header tab inside the Feeds modal (SetFeedsModalMode(Project)); the modal itself opens from the Feeds tab.
controls:
- request-feeds-suggestion (vendor-lut, per-row recommended feed/DOC columns)
- apply-all-feeds (per-row Apply → ApplyFeedsAll)
- apply-feeds-project-selected (⚡ Apply selected → ApplyFeedsProjectSelected)
- apply-feeds-project (⚡⚡ Apply all toolpaths → ApplyFeedsProject)
- toggle-feeds-project-row (per-row checkbox → ToggleFeedsProjectRow)
- set-feeds-project-select-all (Select all → SetFeedsProjectSelectAll)
- set-feeds-project-sort (Order/Speedup/Name → SetFeedsProjectSort)
- toggle-feeds-project-scatter (Show feed-RPM scatter → SetFeedsProjectScatter)
- set-spindle-strategy (shared head row → SetSpindleStrategy)
reads-state: per-toolpath CurrentValues + FeedsExplain for every enabled toolpath; modal.project_sort, project_selected, project_show_scatter; machine
writes-state: (via events) per-toolpath feed/plunge/rpm/doc/stepover; modal.project_sort, project_selected, project_show_scatter; post_config.spindle_strategy
confusable-with: optimize-project (a SEPARATE project-wide rollup table that also has a bottleneck callout, per-row Δ, cycle saving, and batch Apply — but driven by the sim-based optimizer, not the LUT)
recommendation-sources-touched: vendor-lut
health: yellow — clean single table, but it is a near-twin of optimize-project (same bottleneck-callout + per-row Δ + select/apply-selected idiom) sourced from a different engine; user cannot tell from layout which "recommendation" they are batch-applying (P4/P7).
