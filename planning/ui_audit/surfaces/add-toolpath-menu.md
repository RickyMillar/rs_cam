surface: Add Toolpath Menu
file: crates/rs_cam_viz/src/ui/toolpath_panel.rs
kind: menu
job: Pick an operation type to add to a setup, grouped 2.5D vs 3D, with unavailable types disabled by geometry requirement.
opens-from: "+ Add" button in the Operations Queue Panel (per-setup or single-setup footer)
controls:
  - add-toolpath
reads-state: session.models (has_mesh / has_polygons), OperationType::ALL_2D / ALL_3D, op.spec().geometry
writes-state: emits Select(Setup) + AddToolpath events
confusable-with: none
recommendation-sources-touched: none
health: green — single purpose, grouped by 2D/3D concern (P2), disabled items carry on-hover reason for why they can't be used (P5 done right).
