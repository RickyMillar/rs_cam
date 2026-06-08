surface: Toolpath Card Context Menu
file: crates/rs_cam_viz/src/ui/toolpath_panel.rs
kind: menu
job: Offer the full set of per-toolpath actions (generate, inspect, isolate, hide, enable, duplicate, move, delete) via right-click.
opens-from: Right-click on a toolpath card in the Operations Queue Panel
controls:
  - generate-toolpath
  - inspect-toolpath-in-simulation
  - toggle-isolate-toolpath
  - toggle-toolpath-visibility
  - toggle-toolpath-enabled
  - duplicate-toolpath
  - move-toolpath-up
  - move-toolpath-down
  - delete-toolpath
reads-state: viewport.isolate_toolpath, card enabled/visible/has_result snapshot
writes-state: emits AppEvents only
confusable-with: Operations Queue Panel inline quick actions (Generate ▶ / Sim / eye / isolate bullseye all duplicated here)
recommendation-sources-touched: none
health: yellow — clean menu but every action except Move/Delete is also reachable as an inline affordance on the same card (P1 one-concern-many-homes; P6 dead duplicates of generate/inspect/isolate/hide).
