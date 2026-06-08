surface: Params Tab
file: crates/rs_cam_viz/src/ui/properties/mod.rs (ToolpathTab::Params ~2760-3004) + operations/*.rs draw_*_params
kind: tab
job: Edit the geometry/strategy parameters of the selected operation (stepover, depth, depth-per-pass, feeds, plus op-specific options) with inline LUT Suggest pills and a pattern diagram.
opens-from: Params tab in the Toolpath Tab Bar (default)
controls:
  - set-stepover
  - set-depth
  - set-depth-per-pass
  - set-feed-rate
  - set-plunge-rate
  - set-spindle-rpm
  - request-feeds-suggestion-all
  - request-feeds-suggestion-stepover
  - request-feeds-suggestion-doc
  - request-feeds-suggestion-feed-rate
  - request-feeds-suggestion-plunge-rate
  - request-feeds-suggestion-spindle-rpm
  - apply-stale-default-fix
  - set-op-pattern
  - set-cut-direction-climb
  - set-finishing-passes
  - set-tab-count
  - set-adaptive3d-entry-style
  - set-clearing-strategy
  - set-min-cut-radius
  - set-tolerance
  - (many op-specific fields — see capabilities list)
reads-state: entry.operation (all per-op config fields), entry.feeds_result, stale_default_defects, tool_configs (for feeds_result_for_operation), material, machine, workholding, spindle_strategy
writes-state: entry.operation.* fields directly (DragValues + dv_pill pills); apply_feeds_result_to_op on "Suggest all (LUT)"; apply_stale_default_to_op; entry.stale_since on every suggest/fix
confusable-with: Feeds Tab (same feed/plunge/rpm/stepover/doc editable there too via Suggest buttons) and Feeds Modal (third editor of the same fields)
recommendation-sources-touched: suggest-icon (inline ⚡ dv_pill pills), vendor-lut (pill colour green=VendorLut amber=formula/edge-floor; "Suggest all (LUT)"), sim-feedback (stale-default Fix banner)
health: red — feeds/speeds values have three homes (Params pills, Feeds card buttons, Feeds modal) all writing the same operation fields (P1); the tab is a long flat grid of mixed concerns — geometry, feeds, strategy, entry-style — with no internal grouping (P2/P3); recompputes feeds_result independently from the Feeds tab.
