surface: keepout-properties-panel
file: crates/rs_cam_viz/src/ui/properties/setup.rs
kind: panel
job: Edit one keep-out zone's name, enabled flag, position, and size.
opens-from: Selecting a keep-out row in setup-properties-panel (right properties dock).
controls:
  - set-keepout-name
  - toggle-keepout-enabled
  - set-keepout-position
  - set-keepout-size
reads-state: zone (name, enabled, origin_x/y, size_x/y)
writes-state: zone.name, zone.enabled, zone.origin_x/y, zone.size_x/y; emits FixtureChanged
confusable-with: fixture-properties-panel (near-identical name + enabled + position/size grids; this one is XY-only, no Z)
recommendation-sources-touched: none
health: yellow — clean single purpose but visually indistinguishable from fixture-properties-panel except the missing Z fields; a user cannot tell which editor they are in from layout alone (P4).
