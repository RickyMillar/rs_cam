surface: fixture-properties-panel
file: crates/rs_cam_viz/src/ui/properties/setup.rs
kind: panel
job: Edit one fixture's name, kind, enabled flag, position, size, and clearance.
opens-from: Selecting a fixture row in setup-properties-panel (right properties dock).
controls:
  - set-fixture-name
  - set-fixture-kind
  - toggle-fixture-enabled
  - set-fixture-position
  - set-fixture-size
  - set-fixture-clearance
reads-state: fixture (name, kind, enabled, origin_x/y/z, size_x/y/z, clearance)
writes-state: fixture.name, fixture.kind, fixture.enabled, fixture.origin_x/y/z, fixture.size_x/y/z, fixture.clearance; emits FixtureChanged
confusable-with: keepout-properties-panel (near-identical layout: name + enabled + position grid + size grid)
recommendation-sources-touched: none
health: yellow — single-purpose and clean per field, but is a near-clone of keepout-properties-panel; the two read as the same form doing different jobs (P4) and are a flat field wall (P3).
