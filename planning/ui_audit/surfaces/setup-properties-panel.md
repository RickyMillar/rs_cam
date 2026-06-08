surface: setup-properties-panel
file: crates/rs_cam_viz/src/ui/properties/setup.rs
kind: panel
job: Edit the selected setup's orientation, work datum, model scoping, fixtures, and keep-out zones.
opens-from: Selecting a Setup card in setup-list-panel (right properties dock).
controls:
  - rename-setup
  - set-face-up
  - set-z-rotation
  - setup-two-sided
  - set-xy-datum-method
  - set-z-datum-method
  - set-z-datum-offset
  - set-datum-notes
  - scope-setup-models
  - select-fixture
  - add-fixture
  - delete-fixture
  - select-keepout
  - add-keepout
  - delete-keepout
reads-state: setup_data (name, face_up, z_rotation, fixtures, keep_out_zones), setup_rt.datum (xy_method, z_method, notes), setup_rt.model_ids, pin_count (stock.alignment_pins.len), has_flip_axis, all_models
writes-state: setup_data.name, setup_data.face_up, setup_data.z_rotation, setup_rt.datum.xy_method, setup_rt.datum.z_method, setup_rt.datum.notes, setup_rt.model_ids; emits FixtureChanged/PreviewOrientation/SetupTwoSided/RenameSetup
confusable-with: setup-list-panel (orientation/datum shown as read-only chips there); alignment-pins-section (the "Add alignment pins for this flip" / two-sided button also lives in stock panel)
recommendation-sources-touched: none
health: yellow — coherent "setup config" home, but five sub-concerns (orientation, datum, models, fixtures, keep-out) are a flat stack with no summary->detail progression (P3); the "Add alignment pins for this flip" button duplicates the two-sided action that authoritatively lives in alignment-pins-section (P1), and pin state is read here but edited in the Stock panel (split home, P2).
