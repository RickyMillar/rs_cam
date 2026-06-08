surface: Diagnostics Ribbon
file: crates/rs_cam_viz/src/ui/properties/mod.rs (header section ~2665-2743, render_diagnostic_row ~2147)
kind: inline-widget
job: Surface per-toolpath diagnostics (validation, geometry, tool-load gates, workflow notices) in three tiers — actionable, stateful, collapsed hints — with optional one-click fixes.
opens-from: Rendered in the Toolpath Properties Header below the Generate button, on every tab
controls:
  - apply-stale-default-fix
  - show-toolpath-diagnostics
reads-state: collect_diagnostics(entry, tool, stale_default_defects, height_ctx, load_verdict), entry.auto_regen + status (workflow.needs_generation synthesised), diagnostic state/severity/category/confidence/evidence
writes-state: entry.operation via ApplyStaleDefault fix path; entry.stale_since
confusable-with: Params-tab stale-default Fix banner (same StaleDefault data, rendered twice — once as a yellow banner in Params, once as actionable rows here); Dressups tab badge (derives from this same diagnostic set)
recommendation-sources-touched: sim-feedback (load_verdict gates + NeedsSimulation/StaleEvidence tiers), suggest-icon (Fix buttons)
health: yellow — strong tiered provenance design (P7), but stale-default fixes appear both here AND as a separate banner in the Params tab (P1/P6), and the ribbon sits in the always-on header mixed with unrelated IO fields (P2).
