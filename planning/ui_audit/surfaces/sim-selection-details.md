surface: Inspector › Selection details
file: crates/rs_cam_viz/src/ui/sim_diagnostics.rs
kind: panel
job: Show the active semantic item's label, kind, XY/Z bbox, and first 6 params.
opens-from: CollapsingHeader "Selection details" in the Inspector right panel (default closed)
controls:
  - read-active-semantic-item
reads-state: sim.active_semantic_item(), sim.debug.pinned_semantic_item, active.item.{kind,label,xy_bbox,z_min,z_max,params}
writes-state: none
confusable-with: sim-selected-span-section (also a "what is selected right now" detail block, but scoped to spans not semantic items), Generation Metrics (also reads the active trace)
recommendation-sources-touched: none
health: yellow — read-only detail card that overlaps conceptually with the Selected-span section; two "what's selected" panels (semantic-item vs structural-span) in the same right panel is a P4/P2 confusable.
