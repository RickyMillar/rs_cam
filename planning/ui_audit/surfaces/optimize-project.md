surface: optimize-project
file: crates/rs_cam_viz/src/ui/optimize_project.rs
kind: modal
job: Roll up the sim-based optimizer across every enabled toolpath — baseline vs optimized cycle time, bottleneck callout, per-row cycle saving + verdict — and batch-apply selected candidates, then show a reconciled post-apply project sim.
opens-from: AppEvent::OpenOptimizeProject from menu_bar "Optimize project…" (menu_bar.rs:161, enabled only when sim has results) and sim_diagnostics "⚡ Optimize all N exceeding toolpaths" (sim_diagnostics.rs:666).
controls:
- request-optimize-suggestion (sim-feedback, per-row recommended delta + cycle saving)
- toggle-optimize-project-row (per-row checkbox → ToggleOptimizeProjectRow; disabled when no safe candidate)
- apply-optimize-project (Apply selected → ApplyOptimizeProject)
- close-optimize-project (Close/Cancel → CloseOptimizeProject)
reads-state: optimize_project.status (Loading/Failed/Ready/Reconciling/Reconciled); ProjectOptimizeReport (per_toolpath outcomes, baseline_cycle_time_s, bottleneck_index); view.row_selected
writes-state: (via ApplyOptimizeProject) operation params for selected toolpaths' first_safe candidate
confusable-with: feeds-modal-project (a near-identical project rollup table: bottleneck callout + per-row Δ + cycle/speedup + select + Apply selected, but LUT-sourced not sim-sourced)
recommendation-sources-touched: sim-feedback
health: yellow — clean state machine and single job, but it is structurally the twin of feeds-modal-project; two project-wide "apply recommendations to many toolpaths" rollups from different engines, with no shared visual cue of which engine produced the row (P1/P4/P7).
