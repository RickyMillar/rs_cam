surface: setup-list-panel
file: crates/rs_cam_viz/src/ui/setup_panel.rs
kind: panel
job: Browse setups, stock summary, project readiness, and project-wide diagnostics, selecting items to edit elsewhere.
opens-from: Left dock of the Setup workspace (always visible in that workspace).
controls:
  - select-stock
  - add-setup
  - select-setup
  - select-model
  - reload-model
  - delete-model
  - view-stock-effective-dims (read-only)
  - view-project-readiness (read-only)
  - view-project-diagnostics (read-only)
  - view-estimated-cycle-time (read-only)
reads-state: session.stock_config (x,y,z, alignment_pins), session.list_setups (name, face_up, z_rotation, fixtures, keep_out_zones), gui.setup_rt[].datum.xy_method, session.toolpath_configs (enabled, operation.feed_rate), gui.toolpath_rt[].result.stats.cutting_distance, session.tools, simulation.results (boundaries, cut_trace), simulation.checks.rapid_collisions, selection
writes-state: none directly — emits AppEvent (Select, AddSetup, ReloadModel, RemoveModel)
confusable-with: setup-properties-panel (both show orientation/datum chips vs editable fields)
recommendation-sources-touched: sim-feedback
health: yellow — single navigational job, but mixes setup list, stock summary card, project readiness summary, project-diagnostics card (sim-feedback), and a Models collapsing list; four unrelated concerns share one wall (P2/P3). The stock summary and orientation/datum chips duplicate values authored in stock-properties-panel and setup-properties-panel (P1).
