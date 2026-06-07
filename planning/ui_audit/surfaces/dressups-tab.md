surface: Dressups Tab
file: crates/rs_cam_viz/src/ui/properties/mod.rs (ToolpathTab::Mods ~3111-3266, draw_dressup_params ~3525)
kind: tab
job: Toggle and configure post-processing dressups (entry/exit, path quality, optimization, safety) plus the per-toolpath machining boundary.
opens-from: Dressups tab (labelled "Dressups", enum ToolpathTab::Mods) in the Toolpath Tab Bar
controls:
  - reset-dressups-to-recommended
  - set-dressup-entry-style
  - set-ramp-angle
  - set-helix-radius
  - set-helix-pitch
  - toggle-lead-in-out
  - set-lead-radius
  - toggle-arc-fitting
  - set-arc-tolerance
  - toggle-dogbone
  - set-dogbone-angle
  - toggle-link-moves
  - set-link-max-distance
  - set-link-feed-rate
  - toggle-feed-optimization
  - set-feed-max-rate
  - set-feed-ramp-rate
  - toggle-optimize-rapid-order
  - set-retract-strategy
  - toggle-boundary
  - toggle-boundary-inherit
  - set-boundary-source
  - set-boundary-containment
  - set-boundary-offset
reads-state: entry.dressups (DressupConfig), entry.operation (op_type spec ui_process_role + dressup_policy.strip_all_reason), entry.boundary, entry.boundary_inherit, entry.stock_source, height_ctx
writes-state: entry.dressups.* (all fields), entry.boundary.*, entry.boundary_inherit
confusable-with: Params Tab for adaptive3d (adaptive3d has its OWN entry_style Plunge/Helix/Ramp + ramp_angle/helix_radius/helix_pitch on Adaptive3dConfig, distinct from DressupConfig.entry_style here)
recommendation-sources-touched: none ("Reset to recommended" applies role-based DressupConfig::for_role defaults, not a feeds/LUT recommendation)
health: yellow — well grouped into 4 named sections + boundary (P2 done well), but entry-style/ramp/helix duplicates the adaptive3d Params controls under different state fields (P4 confusable), and incompatible dressups are greyed with prose reasons (acceptable P5).
